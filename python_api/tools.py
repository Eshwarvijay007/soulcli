"""
This module contains the tools that the SoulCLI backend can use, 
such as the web search tool.
"""

import os
import google.generativeai as genai

# Configure the Gemini API key
genai.configure(api_key=os.environ["GEMINI_API_KEY"])

def web_search(query: str):
    """
    Performs a web search using the Gemini API's built-in Google Search tool.

    Args:
        query: The search query.

    Returns:
        The search results as a string.
    """
    try:
        model = genai.GenerativeModel('gemini-pro')
        result = model.generate_content(
            query,
            tools=['google_search'],
        )
        return result.text
    except Exception as e:
        return f"Error performing web search: {e}"
