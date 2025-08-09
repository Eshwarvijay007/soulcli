import os
from typing import List
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

if __name__ == "__main__":
    import uvicorn
    host = os.getenv("HOST", "127.0.0.1")
    port = int(os.getenv("PORT", "8000"))
    uvicorn.run(app, host=host, port=port)
