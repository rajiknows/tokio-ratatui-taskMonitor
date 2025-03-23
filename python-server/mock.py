import asyncio
import json
import random
import time
import websockets
import uuid
import logging

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("task_server")

# Generate some random task IDs
TASK_IDS = [str(uuid.uuid4())[:8] for _ in range(5)]

# Different status types a task can have
STATUSES = ["running", "waiting", "completed", "failed"]

# Generic log messages to simulate task execution
LOG_MESSAGES = [
    "Loading reference genome...",
    "Aligning sequences...",
    "Processing chromosome {}...",
    "Analyzing variants...",
    "Calculating statistics...",
    "Optimizing memory usage...",
    "Generating report...",
    "Validating results...",
    "Waiting for resources...",
    "Executing subtask {}..."
]

# Task event generators
async def generate_task_init(task_id, index):
    return {
        "event_type": "TaskInit",
        "task_id": task_id,
        "name": f"Genomic Analysis Task {index+1}",
        "created_at": time.time(),
        "priority": random.randint(1, 5)
    }

async def generate_task_update(task_id):
    return {
        "event_type": "TaskUpdate",
        "task_id": task_id,
        "status": random.choice(STATUSES),
        "progress": round(random.random(), 2),
        "timestamp": time.time()
    }

async def generate_task_metrics(task_id):
    return {
        "event_type": "TaskMetrics",
        "task_id": task_id,
        "cpu_usage": round(random.uniform(0.1, 100.0), 1),
        "memory_usage": round(random.uniform(50, 8192), 1),
        "disk_io": round(random.uniform(0.1, 50.0), 1),
        "timestamp": time.time()
    }

async def generate_task_logs(task_id):
    msg = random.choice(LOG_MESSAGES)
    if "{}" in msg:
        msg = msg.format(random.randint(1, 22))
    return {
        "event_type": "TaskLogs",
        "task_id": task_id,
        "log_level": random.choice(["INFO", "DEBUG", "WARNING", "ERROR"]),
        "message": msg,
        "timestamp": time.time()
    }

async def generate_task_terminate(task_id):
    return {
        "event_type": "TaskTerminate",
        "task_id": task_id,
        "exit_code": random.choice([0, 1]),
        "reason": "Completed successfully" if random.random() > 0.2 else "Failed with error",
        "timestamp": time.time()
    }

async def handler(websocket):
    client_id = str(uuid.uuid4())[:6]
    logger.info(f"Client {client_id} connected")

    # Dictionary to track active tasks
    active_tasks = {}

    # Start by initializing the first 3 tasks
    for i in range(3):
        task_id = TASK_IDS[i]
        active_tasks[task_id] = True
        init_message = await generate_task_init(task_id, i)
        try:
            await websocket.send(json.dumps(init_message))
            logger.info(f"Initialized task {task_id} for client {client_id}")
        except websockets.exceptions.ConnectionClosed:
            logger.warning(f"Connection closed while initializing tasks for client {client_id}")
            return
        await asyncio.sleep(0.5)

    # Counter to track when to add the remaining tasks
    counter = 0

    try:
        while True:
            # Add the remaining tasks after some time
            if counter == 20:  # After about 20 seconds
                for i in range(3, 5):
                    task_id = TASK_IDS[i]
                    active_tasks[task_id] = True
                    init_message = await generate_task_init(task_id, i)
                    await websocket.send(json.dumps(init_message))
                    logger.info(f"Initialized additional task {task_id} for client {client_id}")
                    await asyncio.sleep(0.5)

            # Randomly select an active task
            active_task_ids = [tid for tid, active in active_tasks.items() if active]
            if not active_task_ids:
                logger.info(f"No active tasks for client {client_id}, resetting simulation")
                # Reset all tasks instead of breaking
                for task_id in TASK_IDS:
                    active_tasks[task_id] = True
                counter = 0
                continue

            task_id = random.choice(active_task_ids)

            # Randomly select an event type to send
            event_type = random.choice(["update", "metrics", "logs"])
            message = None

            if event_type == "update":
                message = await generate_task_update(task_id)
                # Randomly terminate tasks
                if message["status"] in ["completed", "failed"]:
                    if random.random() < 0.2:  # 20% chance to terminate
                        active_tasks[task_id] = False
                        term_message = await generate_task_terminate(task_id)
                        try:
                            await websocket.send(json.dumps(term_message))
                            logger.info(f"Task {task_id} terminated for client {client_id}")
                        except websockets.exceptions.ConnectionClosed:
                            logger.warning(f"Connection closed for client {client_id}")
                            return
            elif event_type == "metrics":
                message = await generate_task_metrics(task_id)
            else:  # logs
                message = await generate_task_logs(task_id)

            try:
                if message:
                    await websocket.send(json.dumps(message))
            except websockets.exceptions.ConnectionClosed:
                logger.warning(f"Connection closed for client {client_id}")
                return

            # Shorter delay between messages
            await asyncio.sleep(random.uniform(0.3, 1.0))
            counter += 1

            # Check if all tasks terminated
            if not any(active_tasks.values()):
                logger.info(f"All tasks completed for client {client_id}, resetting...")
                # Reset the simulation after all tasks complete
                for task_id in TASK_IDS:
                    active_tasks[task_id] = True
                counter = 0

    except websockets.exceptions.ConnectionClosed:
        logger.info(f"Client {client_id} disconnected")
    except Exception as e:
        logger.error(f"Error in handler for client {client_id}: {str(e)}")

async def main():
    logger.info("Starting WebSocket server on localhost:8765")
    async with websockets.serve(handler, "localhost", 8765):
        await asyncio.Future()  # Run forever

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        logger.info("Server shutdown requested")
    except Exception as e:
        logger.error(f"Server error: {str(e)}")
