import os
import httpx

class LlmClient:
    def __init__(self):
        self.mode = os.getenv("LLM_MODE", "chat_completions")
        self.gemini_key = os.getenv("GEMINI_API_KEY")
        self.gemini_model = os.getenv("GEMINI_MODEL", "gemini-pro")

    async def chat(self, prompt: str, history: list[str]):
        if self.mode == "gemini_http":
            return await self._chat_gemini_http(prompt, history)
        # ... other modes (openai, ollama, etc.) ...

    async def _chat_gemini_http(self, prompt: str, history: list[str]):
        url = f"https://generativelanguage.googleapis.com/v1beta/models/{self.gemini_model}:generateContent?key={self.gemini_key}"
        # Merge history + current prompt
        combined_text = "\n".join(history[-8:] + [prompt])
        payload = {
            "contents": [
                {"parts": [{"text": combined_text}]}
            ]
        }
        async with httpx.AsyncClient(timeout=60) as client:
            r = await client.post(url, json=payload)
            r.raise_for_status()
            data = r.json()

        # Extract text from Gemini response
        try:
            text = data["candidates"][0]["content"]["parts"][0]["text"]
        except KeyError:
            text = str(data)
        return {"text": text}


def tag_emotion(text: str) -> str:
    # TODO: Implement actual emotion tagging
    return "neutral"
