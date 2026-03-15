from __future__ import annotations

import asyncio
from typing import Any

from fastapi import FastAPI, HTTPException, Query, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from starlette.concurrency import run_in_threadpool

from proteus.api.schemas import (
    AssembleRequest,
    ControlRequest,
    ImportRunRequest,
    SimulationConfigModel,
)
from proteus.defaults import build_defaults_payload
from proteus.spec import assemble_program, normalize_assembly
from proteus.worker import RunManager

STREAM_FRAME_TIMEOUT = 1 / 30


def _manager(app: FastAPI) -> RunManager:
    return app.state.run_manager


def _wrap_error(exc: Exception) -> HTTPException:
    message = str(exc)
    status = 404 if "No active run" in message else 400
    return HTTPException(status_code=status, detail=message)


def create_app() -> FastAPI:
    app = FastAPI(title="Proteus API", version="0.1.0")
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["http://localhost:5173", "http://127.0.0.1:5173"],
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    @app.on_event("startup")
    def startup() -> None:
        app.state.run_manager = RunManager()

    @app.on_event("shutdown")
    def shutdown() -> None:
        app.state.run_manager.close()

    @app.get("/api/defaults")
    def get_defaults() -> dict[str, Any]:
        return build_defaults_payload()

    @app.post("/api/assemble")
    def assemble(request: AssembleRequest) -> dict[str, Any]:
        try:
            bytecode = assemble_program(request.source)
            return {
                "bytecode": bytecode,
                "disassembly": normalize_assembly(request.source),
            }
        except ValueError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

    @app.post("/api/runs")
    def create_run(request: SimulationConfigModel) -> dict[str, Any]:
        try:
            return _manager(app).create_run(request.model_dump())
        except Exception as exc:
            raise _wrap_error(exc) from exc

    @app.get("/api/runs/current")
    def get_current_run() -> dict[str, Any]:
        try:
            return _manager(app).get_current()
        except Exception as exc:
            raise _wrap_error(exc) from exc

    @app.post("/api/runs/current/control")
    def control_run(request: ControlRequest) -> dict[str, Any]:
        try:
            return _manager(app).control(
                request.action,
                steps=request.steps,
                target_tps=request.target_tps,
                include_target_tps=request.action == "set_speed",
            )
        except Exception as exc:
            raise _wrap_error(exc) from exc

    @app.get("/api/runs/current/viewport")
    def get_viewport(
        origin_x: int = Query(0),
        origin_y: int = Query(0),
        width: int = Query(64, ge=1),
        height: int = Query(64, ge=1),
        overlay: str = Query("occupancy"),
    ) -> dict[str, Any]:
        try:
            return _manager(app).viewport(origin_x, origin_y, width, height, overlay)
        except Exception as exc:
            raise _wrap_error(exc) from exc

    @app.get("/api/runs/current/cells/{x}/{y}")
    def get_cell_detail(x: int, y: int) -> dict[str, Any]:
        try:
            return _manager(app).cell_detail(x, y)
        except Exception as exc:
            raise _wrap_error(exc) from exc

    @app.get("/api/runs/current/export")
    def export_run() -> JSONResponse:
        try:
            archive = _manager(app).export_run()
            return JSONResponse(
                archive,
                headers={"Content-Disposition": 'attachment; filename="proteus-run.json"'},
            )
        except Exception as exc:
            raise _wrap_error(exc) from exc

    @app.post("/api/runs/import")
    def import_run(request: ImportRunRequest) -> dict[str, Any]:
        try:
            return _manager(app).import_run(request.archive)
        except Exception as exc:
            raise _wrap_error(exc) from exc

    @app.websocket("/api/runs/current/stream")
    async def stream_run(websocket: WebSocket) -> None:
        await websocket.accept()
        viewport = {"origin_x": 0, "origin_y": 0, "width": 64, "height": 64, "overlay": "occupancy"}
        try:
            while True:
                try:
                    message = await asyncio.wait_for(websocket.receive_json(), timeout=STREAM_FRAME_TIMEOUT)
                    if isinstance(message, dict) and message.get("type") == "viewport":
                        viewport = {
                            "origin_x": int(message.get("origin_x", viewport["origin_x"])),
                            "origin_y": int(message.get("origin_y", viewport["origin_y"])),
                            "width": int(message.get("width", viewport["width"])),
                            "height": int(message.get("height", viewport["height"])),
                            "overlay": str(message.get("overlay", viewport["overlay"])),
                        }
                except asyncio.TimeoutError:
                    pass

                try:
                    frame = await run_in_threadpool(
                        _manager(app).frame,
                        viewport["origin_x"],
                        viewport["origin_y"],
                        viewport["width"],
                        viewport["height"],
                        viewport["overlay"],
                    )
                    await websocket.send_json(
                        {
                            "type": "frame",
                            "status": frame["status"],
                            "target_tps": frame["target_tps"],
                            "summary": frame["summary"],
                            "viewport": frame["viewport"],
                        }
                    )
                except Exception as exc:
                    await websocket.send_json({"type": "error", "message": str(exc)})
                    await asyncio.sleep(0.15)
        except WebSocketDisconnect:
            return

    return app
