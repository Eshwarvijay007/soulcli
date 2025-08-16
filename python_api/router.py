import json
import re
from .tools import web_search
from .llm_client import LlmClient

# A list of commands that are considered complex and might benefit from a web search.
COMPLEX_COMMANDS = ["git", "docker", "kubectl", "ffmpeg", "openssl", "rsync"]

# Keywords that indicate the output might be a log file.
LOG_KEYWORDS = ["error", "exception", "failed", "traceback", "warning"]

def format_response(data: dict) -> str:
    """
    Formats the structured data from the LLM into a clean, readable string.
    """
    title = data.get("title", "SoulCLI Analysis")
    command = data.get("command", "")
    # Clean the analysis and recommendations to remove any markdown
    analysis = data.get("analysis", "").replace("**", "")
    recommendations = [rec.replace("**", "") for rec in data.get("recommendations", [])]

    # Build the formatted string
    response = f"✨ {title}\n"
    response += f"{\"-\" * 40}\n\n"
    response += f"➡️ Command:\n   {command}\n\n"
    response += f"{\"-\" * 40}\n\n"
    response += f"💡 Analysis:\n   {analysis}\n\n"
    response += f"{\"-\" * 40}\n\n"
    response += f"✅ Recommendations:\n"
    for i, rec in enumerate(recommendations, 1):
        response += f"   {i}. {rec}\n"
    
    return response

async def get_structured_analysis(client: LlmClient, prompt: str) -> str:
    """
    Calls the LLM with a specific prompt to get a structured analysis in JSON format.
    """
    system_prompt = (
        "You are a helpful assistant for a developer terminal. "
        "Your task is to provide a clear, structured analysis of a command's output. "
        "Return a JSON object with the following keys: 'title', 'command', 'analysis', and 'recommendations'. "
        "The 'recommendations' value should be a list of strings. "
        "IMPORTANT: Do not use any Markdown or other formatting in the JSON values. Use plain text only."
    )
    
    full_prompt = f"{system_prompt}\n\n{prompt}"
    
    result = await client.chat(full_prompt, [])
    text_response = result.get("text", "{}")
    
    try:
        # The model might return the JSON wrapped in ```json ... ```, so we need to strip that.
        if text_response.startswith("```json"):
            text_response = text_response[7:-3].strip()
        json_response = json.loads(text_response)
        return format_response(json_response)
    except json.JSONDecodeError:
        return "Error: Could not decode the analysis from the LLM."

async def route_request(command: str, output: str) -> str:
    """
    Routes the request to the appropriate handler based on the command and its output.
    """
    client = LlmClient()
    command_parts = command.split()
    command_name = command_parts[0]

    # Path 3: Log Analysis
    if any(keyword in output.lower() for keyword in LOG_KEYWORDS):
        for line in output.splitlines():
            if any(keyword in line.lower() for keyword in LOG_KEYWORDS):
                search_query = f"how to fix error: {line}"
                search_result = web_search(search_query)
                prompt = (
                    f"The user ran the command '{command}' and got the following output:\n---\n{output}\n---\n"
                    f"I performed a web search for the error and got this result:\n---\n{search_result}\n---\n"
                    f"Please analyze this and provide recommendations."
                )
                return await get_structured_analysis(client, prompt)
        
        return "Log analysis path, but no specific error line found."

    # Path 2: Complex Command
    if command_name in COMPLEX_COMMANDS:
        search_query = f"documentation for {command}"
        search_result = web_search(search_query)
        prompt = (
            f"The user ran the command '{command}' and got the following output:\n---\n{output}\n---\n"
            f"I performed a web search for the command and got this documentation:\n---\n{search_result}\n---\n"
            f"Please analyze the output based on the documentation and provide recommendations."
        )
        return await get_structured_analysis(client, prompt)

    # Path 1: Simple Analysis (default)
    prompt = (
        f"The user ran the command '{command}' and got the following output:\n---\n{output}\n---\n"
        f"Please provide a simple analysis of the output."
    )
    return await get_structured_analysis(client, prompt)





