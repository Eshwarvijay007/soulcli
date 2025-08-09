import os
from typing import List
from fastapi import FastAPI
from pydantic import BaseModel
from dotenv import load_dotenv
from llm_client import LlmClient, tag_emotion

load_dotenv()

app = FastAPI()
client = LlmClient()

class Query(BaseModel):
    input: str
    history: List[str] = []

@app.get("/health")
def health():
    return {"ok": True}

@app.post("/query")
async def query_llm(q: Query):
    result = await client.chat(q.input, q.history)
    text = (result.get("text") or "").strip()
    emotion = tag_emotion(text)
    return {"text": text, "emotion": emotion}

if __name__ == "__main__":
    import uvicorn
    host = os.getenv("HOST", "127.0.0.1")
    port = int(os.getenv("PORT", "8000"))
    uvicorn.run(app, host=host, port=port)
