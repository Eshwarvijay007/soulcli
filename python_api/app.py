import os
from typing import List
import json
from fastapi import FastAPI
from pydantic import BaseModel
from dotenv import load_dotenv
from llm_client import LlmClient, tag_emotion
from fastapi.middleware.cors import CORSMiddleware

load_dotenv()

app = FastAPI()

# Allow local CLI to call the API during dev
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Single client instance
client = LlmClient()

class Query(BaseModel):
    input: str
    history: List[str] = []

class RouteIn(BaseModel):
    input: str
    history: List[str] = []

class RouteOut(BaseModel):
    mode: str
    framed: str
    note: str = ""

@app.get("/health")
async def health():
    return {"ok": True}

@app.post("/query")
async def query_llm(q: Query):
    # Delegates to client; returns text + emotion tag
    result = await client.chat(q.input, q.history)
    text = (result.get("text") or "").strip()
    emotion = tag_emotion(text)
    return {"text": text, "emotion": emotion}

@app.post("/route", response_model=RouteOut)
async def route(q: RouteIn):
    """LLM-powered prompt router: returns mode, framed prompt, and a short note."""
    sys_instructions = (
        "You are a prompt router for a developer terminal assistant.\n"
        "Decide a mode (one of: cli_help, philosophy, emotional, concise).\n"
        "Then create a 'framed' prompt that instructs the assistant how to answer.\n"
        "Rules by mode:\n"
        "- cli_help: exact commands, concise (<= 2 lines), prefer one-liners, numbered steps if needed.\n"
        "- philosophy: thoughtful yet succinct (<= 5 lines).\n"
        "- emotional: short vivid response (6–10 lines), empathetic tone.\n"
        "- concise: answer in <= 2 lines.\n"
        "Also detect funny CLI typos (e.g., 'gti' -> 'git') and bake a gentle, humorous correction into the framed prompt so the answer includes the corrected command and a playful quip.\n"
        "Return STRICT JSON with keys: mode, framed, note. No extra text."
    )

    history_lines = "\n".join(q.history[-8:])
    router_prompt = (
        f"[SYSTEM]\n{sys_instructions}\n\n"
        f"[HISTORY]\n{history_lines}\n\n"
        f"[USER_RAW]\n{q.input}"
    )

    result = await client.chat(router_prompt, [])
    text = (result.get("text") or "").strip()
    # Try to strip code fences if the model wrapped JSON
    if text.startswith("```"):
        text = text.strip('`')
        # remove possible language tag
        if "\n" in text:
            text = text.split("\n", 1)[1]

    try:
        data = json.loads(text)
        mode = str(data.get("mode", "concise"))
        framed = str(data.get("framed", q.input))
        note = str(data.get("note", ""))
    except Exception:
        # Fallback: default concise
        mode = "concise"
        framed = f"[SYSTEM]\nAnswer succinctly in <= 2 lines.\n\n[USER]\n{q.input}"
        note = ""

    return {"mode": mode, "framed": framed, "note": note}

if __name__ == "__main__":
    import uvicorn
    host = os.getenv("HOST", "127.0.0.1")
    port = int(os.getenv("PORT", "8000"))
    uvicorn.run(app, host=host, port=port)
