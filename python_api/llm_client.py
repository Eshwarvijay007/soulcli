# python_api/llm_client.py
import os
import httpx

MODEL = os.getenv("GEMINI_MODEL", "models/gemini-1.5-flash")

class LlmClient:
    async def chat(self, prompt: str, history):
        return await self._chat_gemini_http(prompt, history or [])

    async def _chat_gemini_http(self, prompt: str, history):
        gemini_api_key = os.getenv("GEMINI_API_KEY")
        if not gemini_api_key:
            raise RuntimeError("GEMINI_API_KEY not set")

        # Map history strings to Gemini "contents"
        contents = []
        for h in history[-10:]:
            contents.append({"role": "user", "parts": [{"text": h}]})
        contents.append({"role": "user", "parts": [{"text": prompt}]})

        url = f"https://generativelanguage.googleapis.com/v1beta/{MODEL}:generateContent?key={gemini_api_key}"
        payload = { "contents": contents }

        async with httpx.AsyncClient(timeout=30) as client:
            r = await client.post(url, json=payload, headers={"Content-Type": "application/json"})
            try:
                r.raise_for_status()
            except httpx.HTTPStatusError as e:
                # log full body for clues (key restrictions, quota, etc.)
                raise httpx.HTTPStatusError(f"{e} | body={r.text}", request=e.request, response=e.response)
            data = r.json()
            text = ""
            for cand in (data.get("candidates") or []):
                parts = ((cand.get("content") or {}).get("parts") or [])
                for p in parts:
                    text += p.get("text", "")
            return {"text": text or "(no text)"}

def tag_emotion(text: str) -> str:
    t = (text or "").lower()
    if "error" in t or "fail" in t:
        return "alert"
    if "great" in t or "awesome" in t:
        return "happy"
    return "neutral"
