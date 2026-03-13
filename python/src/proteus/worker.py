from __future__ import annotations

import multiprocessing as mp
import queue
import threading
import time
import uuid
from typing import Any

from proteus.engine import SimulationSession, simulation_config_from_dict


def _response_ok(request_id: str, payload: Any) -> dict[str, Any]:
    return {"request_id": request_id, "ok": True, "payload": payload}


def _response_error(request_id: str, message: str) -> dict[str, Any]:
    return {"request_id": request_id, "ok": False, "error": message}


def _schedule_next_tick_deadline(now: float, target_tps: float | None) -> float:
    if target_tps is None or target_tps <= 0:
        return now
    return now + (1.0 / target_tps)


def _command_timeout(
    *,
    has_session: bool,
    status: str,
    target_tps: float | None,
    next_tick_deadline: float,
    now: float,
) -> float:
    if not has_session or status != "playing":
        return 0.25
    if target_tps is None:
        return 0.0
    if target_tps <= 0:
        return 0.25
    return max(0.0, min(0.25, next_tick_deadline - now))


def _worker_main(command_queue: mp.Queue, response_queue: mp.Queue) -> None:
    session: SimulationSession | None = None
    reset_archive: dict[str, Any] | None = None
    status = "idle"
    target_tps: float | None = 4.0
    next_tick_deadline = time.monotonic()

    def current_payload() -> dict[str, Any]:
        if session is None:
            raise RuntimeError("No active run.")
        return {
            "status": status,
            "target_tps": target_tps,
            "summary": session.summary(),
            "config": session.export_archive()["config"],
        }

    while True:
        try:
            timeout = _command_timeout(
                has_session=session is not None,
                status=status,
                target_tps=target_tps,
                next_tick_deadline=next_tick_deadline,
                now=time.monotonic(),
            )
            command = command_queue.get(timeout=timeout)
        except queue.Empty:
            command = None

        if command is not None:
            request_id = command["request_id"]
            kind = command["kind"]
            try:
                if kind == "shutdown":
                    response_queue.put(_response_ok(request_id, {"status": "shutting_down"}))
                    return
                if kind == "create_run":
                    config = simulation_config_from_dict(command["config"])
                    session = SimulationSession.from_config(config)
                    reset_archive = session.clone_reset_archive()
                    status = "paused"
                    next_tick_deadline = time.monotonic()
                    response_queue.put(_response_ok(request_id, current_payload()))
                elif kind == "import_run":
                    archive = command["archive"]
                    session = SimulationSession.from_archive(archive)
                    reset_archive = session.clone_reset_archive()
                    status = archive.get("status", "paused")
                    imported_tps = archive.get("target_tps", target_tps)
                    target_tps = None if imported_tps is None else float(imported_tps)
                    next_tick_deadline = _schedule_next_tick_deadline(time.monotonic(), target_tps if status == "playing" else 0.0)
                    response_queue.put(_response_ok(request_id, current_payload()))
                elif session is None:
                    raise RuntimeError("No active run.")
                elif kind == "get_current":
                    response_queue.put(_response_ok(request_id, current_payload()))
                elif kind == "control":
                    action = command["action"]
                    if action == "play":
                        status = "playing"
                        next_tick_deadline = _schedule_next_tick_deadline(time.monotonic(), target_tps)
                    elif action == "pause":
                        status = "paused"
                    elif action == "step":
                        status = "paused"
                        session.advance(int(command.get("steps", 1)))
                    elif action == "reset":
                        status = "paused"
                        if reset_archive is None:
                            reset_archive = session.clone_reset_archive()
                        session = SimulationSession.from_archive(reset_archive)
                    elif action == "set_speed":
                        raw_tps = command.get("target_tps", target_tps)
                        target_tps = None if raw_tps is None else max(0.0, float(raw_tps))
                        if target_tps == 0:
                            status = "paused"
                        if status == "playing":
                            next_tick_deadline = _schedule_next_tick_deadline(time.monotonic(), target_tps)
                    else:
                        raise RuntimeError(f"Unknown control action: {action}")
                    response_queue.put(_response_ok(request_id, current_payload()))
                elif kind == "viewport":
                    payload = session.viewport(
                        origin_x=int(command.get("origin_x", 0)),
                        origin_y=int(command.get("origin_y", 0)),
                        width=int(command.get("width", 64)),
                        height=int(command.get("height", 64)),
                        overlay=str(command.get("overlay", "occupancy")),
                    )
                    response_queue.put(_response_ok(request_id, payload))
                elif kind == "frame":
                    response_queue.put(
                        _response_ok(
                            request_id,
                            {
                                "status": status,
                                "target_tps": target_tps,
                                "summary": session.summary(),
                                "viewport": session.viewport(
                                    origin_x=int(command.get("origin_x", 0)),
                                    origin_y=int(command.get("origin_y", 0)),
                                    width=int(command.get("width", 64)),
                                    height=int(command.get("height", 64)),
                                    overlay=str(command.get("overlay", "occupancy")),
                                ),
                            },
                        )
                    )
                elif kind == "cell_detail":
                    payload = session.cell_detail(int(command["x"]), int(command["y"]))
                    response_queue.put(_response_ok(request_id, payload))
                elif kind == "export":
                    archive = session.export_archive()
                    archive["status"] = status
                    archive["target_tps"] = target_tps
                    archive["reset_archive"] = reset_archive
                    response_queue.put(_response_ok(request_id, archive))
                else:
                    raise RuntimeError(f"Unknown worker command: {kind}")
            except Exception as exc:  # pragma: no cover - worker boundary
                response_queue.put(_response_error(request_id, str(exc)))

        if session is not None and status == "playing" and target_tps is None:
            session.advance(128)
            continue

        if session is not None and status == "playing" and target_tps > 0:
            now = time.monotonic()
            if now >= next_tick_deadline:
                session.advance(1)
                next_tick_deadline = _schedule_next_tick_deadline(time.monotonic(), target_tps)


class RunManager:
    def __init__(self) -> None:
        self._ctx = mp.get_context("spawn")
        self._command_queue: mp.Queue = self._ctx.Queue()
        self._response_queue: mp.Queue = self._ctx.Queue()
        self._process = self._ctx.Process(
            target=_worker_main,
            args=(self._command_queue, self._response_queue),
            daemon=True,
        )
        self._process.start()
        self._lock = threading.Lock()

    def close(self) -> None:
        if self._process.is_alive():
            try:
                self._request("shutdown")
            except Exception:
                pass
            self._process.join(timeout=2)

    def _request(self, kind: str, **payload: Any) -> Any:
        with self._lock:
            request_id = str(uuid.uuid4())
            self._command_queue.put({"request_id": request_id, "kind": kind, **payload})
            while True:
                response = self._response_queue.get(timeout=10)
                if response["request_id"] != request_id:
                    continue
                if not response["ok"]:
                    raise RuntimeError(response["error"])
                return response["payload"]

    def create_run(self, config: dict[str, Any]) -> dict[str, Any]:
        return self._request("create_run", config=config)

    def import_run(self, archive: dict[str, Any]) -> dict[str, Any]:
        return self._request("import_run", archive=archive)

    def get_current(self) -> dict[str, Any]:
        return self._request("get_current")

    def control(
        self,
        action: str,
        *,
        steps: int | None = None,
        target_tps: float | None = None,
        include_target_tps: bool = False,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {"action": action}
        if steps is not None:
            payload["steps"] = steps
        if include_target_tps:
            payload["target_tps"] = target_tps
        return self._request("control", **payload)

    def viewport(self, origin_x: int, origin_y: int, width: int, height: int, overlay: str) -> dict[str, Any]:
        return self._request(
            "viewport",
            origin_x=origin_x,
            origin_y=origin_y,
            width=width,
            height=height,
            overlay=overlay,
        )

    def frame(self, origin_x: int, origin_y: int, width: int, height: int, overlay: str) -> dict[str, Any]:
        return self._request(
            "frame",
            origin_x=origin_x,
            origin_y=origin_y,
            width=width,
            height=height,
            overlay=overlay,
        )

    def cell_detail(self, x: int, y: int) -> dict[str, Any]:
        return self._request("cell_detail", x=x, y=y)

    def export_run(self) -> dict[str, Any]:
        return self._request("export")
