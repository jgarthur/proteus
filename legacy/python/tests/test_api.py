import time

from fastapi.testclient import TestClient

from proteus.api.app import create_app


def test_api_run_flow():
    app = create_app()
    with TestClient(app) as client:
        defaults = client.get("/api/defaults")
        assert defaults.status_code == 200
        config = defaults.json()["config"]

        created = client.post("/api/runs", json=config)
        assert created.status_code == 200
        assert created.json()["status"] == "paused"

        current = client.get("/api/runs/current")
        assert current.status_code == 200
        assert current.json()["summary"]["occupied_cells"] >= 1

        viewport = client.get("/api/runs/current/viewport?origin_x=0&origin_y=0&width=8&height=8&overlay=occupancy")
        assert viewport.status_code == 200
        assert len(viewport.json()["cells"]) == 8


def test_api_supports_unlimited_speed():
    app = create_app()
    with TestClient(app) as client:
        config = client.get("/api/defaults").json()["config"]
        client.post("/api/runs", json=config)

        response = client.post("/api/runs/current/control", json={"action": "set_speed", "target_tps": None, "steps": 1})

        assert response.status_code == 200
        assert response.json()["target_tps"] is None


def test_stream_does_not_batch_ticks_at_low_tps():
    app = create_app()
    with TestClient(app) as client:
        config = client.get("/api/defaults").json()["config"]
        client.post("/api/runs", json=config)
        client.post("/api/runs/current/control", json={"action": "set_speed", "target_tps": 1, "steps": 1})

        distinct_ticks: list[int] = []
        with client.websocket_connect("/api/runs/current/stream") as websocket:
            websocket.send_json({"type": "viewport", "origin_x": 0, "origin_y": 0, "width": 8, "height": 8, "overlay": "occupancy"})
            client.post("/api/runs/current/control", json={"action": "play", "steps": 1})

            deadline = time.time() + 3.5
            while time.time() < deadline and (not distinct_ticks or distinct_ticks[-1] < 2):
                payload = websocket.receive_json()
                if payload.get("type") != "frame":
                    continue
                tick = int(payload["summary"]["tick"])
                if not distinct_ticks or tick != distinct_ticks[-1]:
                    distinct_ticks.append(tick)

        assert distinct_ticks
        assert distinct_ticks[-1] >= 2
        assert all((next_tick - current_tick) <= 1 for current_tick, next_tick in zip(distinct_ticks, distinct_ticks[1:]))
