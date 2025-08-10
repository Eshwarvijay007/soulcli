# SoulCLI - A Terminal with a Soul

SoulCLI is a command-line interface that combines the power of a traditional shell with the intelligence of a large language model. It's designed to be a helpful and empathetic companion for developers, providing assistance with shell commands, answering questions, and even offering a bit of encouragement.

## Features

*   **Interactive Shell**: A fully functional shell that can execute any command.
*   **LLM Integration**: Seamlessly integrated with a large language model to provide context-aware help and suggestions.
*   **Command Analysis**: Get explanations and suggestions for shell command outputs.
*   **Smart History**: A history that learns from your commands and provides better suggestions over time.
*   **Autocorrection**: Automatically corrects common typos in shell commands.
*   **Emotional Awareness**: The CLI has a "mood" that reflects the nature of the conversation.

## Project Structure

The project is a monorepo with two main components:

*   `rust_cli/`: A Rust application that implements the command-line interface. It uses `crossterm` and `ratatui` to create the terminal UI.
*   `python_api/`: A Python application that provides an API for the LLM. It uses `fastapi` and communicates with the Gemini API.

```
/
├── python_api/         # Python FastAPI server for LLM interaction
│   ├── app.py          # Main FastAPI application
│   ├── llm_client.py   # Client for the Gemini API
│   └── requirements.txt# Python dependencies
├── rust_cli/           # Rust CLI application
│   ├── src/
│   │   ├── main.rs     # Main application entry point
│   │   └── ...
│   └── Cargo.toml      # Rust dependencies
├── .gitignore          # Files to ignore in git
└── run.sh              # Startup script
```

## Prerequisites

To run this project, you will need:

*   [Rust](https://www.rust-lang.org/tools/install) (latest stable version)
*   [Python](https://www.python.org/downloads/) (3.9 or higher)
*   A [Gemini API Key](https://ai.google.dev/)

## Getting Started

1.  **Clone the repository:**
    ```bash
    git clone <repository_url>
    cd soulcli
    ```

2.  **Set up the Python environment:**
    ```bash
    cd python_api
    python3 -m venv venv
    source venv/bin/activate
    pip install -r requirements.txt
    cd ..
    ```

3.  **Create the `.env` file:**
    Create a file named `.env` in the `python_api` directory and add your Gemini API key to it:
    ```
    GEMINI_API_KEY=your_api_key_here
    ```

4.  **Run the application:**
    The project comes with a startup script that simplifies the process of running both the Rust CLI and the Python API server.
    ```bash
    ./run.sh
    ```
    This will start the Python server in the background and the Rust CLI in the foreground.

## How to Use

Once the application is running, you can interact with it like a regular shell. You can type commands and press `Enter` to execute them.

*   **Ask questions**: You can ask questions directly in the shell. The LLM will try to answer them.
*   **Get help with commands**: After running a command, the LLM will provide an analysis of the output.
*   **Cancel commands**: Press `x` to cancel a running command.
*   **Exit**: Press `Esc` to exit the CLI.

## Contributing

Contributions are welcome! Please feel free to open an issue or submit a pull request.

## License

This project is licensed under the MIT License. See the `LICENSE` file for details.
