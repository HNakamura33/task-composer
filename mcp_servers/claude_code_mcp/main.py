import sys
import logging
from pathlib import Path
import anyio
import json
from dataclasses import asdict
from claude_agent_sdk import query, ClaudeAgentOptions, ClaudeSDKClient, AgentDefinition, ResultMessage
from fastmcp import FastMCP

# stdioモードではログを完全に無効化（HTTPモードではログを有効にする）
if len(sys.argv) > 1 and sys.argv[1] == "serve" and "--http" not in sys.argv:
    logging.disable(logging.CRITICAL)

# FastMCPサーバーを作成
mcp = FastMCP("claude-code-mcp")


def dict_to_agent_definition(agent_dict: dict) -> AgentDefinition:
    """辞書からAgentDefinitionを作成する"""
    return AgentDefinition(
        description=agent_dict["description"],
        prompt=agent_dict["prompt"],
        tools=agent_dict.get("tools"),
        model=agent_dict.get("model"),
    )


def parse_agents(agents_data: dict | None) -> dict[str, AgentDefinition] | None:
    """agents辞書をAgentDefinitionの辞書に変換する"""
    if agents_data is None:
        return None
    return {
        name: dict_to_agent_definition(agent_dict)
        for name, agent_dict in agents_data.items()
    }


def build_role_system_prompt(role: dict) -> str | None:
    """roleからシステムプロンプトを構築する

    roleの情報をClaudeが理解できるシステムプロンプトに変換する。
    ClaudeAgentOptionsには存在しないrole固有のフィールドをここで処理する。
    """
    if not role:
        return None

    parts = []

    # ロール名と説明（ClaudeAgentOptionsにはないフィールド）
    if role.get("name"):
        parts.append(f"# Role: {role['name']}")
    if role.get("description"):
        parts.append(f"\n{role['description']}")

    # スキル（ClaudeAgentOptionsにはないフィールド）
    if role.get("skills"):
        skills_str = ", ".join(role["skills"])
        parts.append(f"\n## Skills: {skills_str}")

    # Note: subagentsはoptions.agentsと重複するため除外

    # ファイル権限情報（ClaudeAgentOptionsのadd_dirsとは異なる詳細な権限）
    file_perms = role.get("file_permissions", {})
    if file_perms.get("allowed_paths"):
        parts.append(f"\n## Allowed Paths: {', '.join(file_perms['allowed_paths'])}")
    if file_perms.get("denied_paths"):
        parts.append(f"\n## Denied Paths (DO NOT ACCESS): {', '.join(file_perms['denied_paths'])}")
    if file_perms.get("read_only_paths"):
        parts.append(f"\n## Read-Only Paths: {', '.join(file_perms['read_only_paths'])}")

    # ツール権限情報（ClaudeAgentOptionsのallowed_toolsとは異なる詳細な権限）
    tool_perms = role.get("tool_permissions", {})
    bash_perms = tool_perms.get("bash", {})
    if bash_perms.get("allowed_commands"):
        parts.append(f"\n## Allowed Bash Commands: {', '.join(bash_perms['allowed_commands'])}")
    if bash_perms.get("blocked_commands"):
        parts.append(f"\n## Blocked Bash Commands: {', '.join(bash_perms['blocked_commands'])}")
    if bash_perms.get("require_confirmation"):
        parts.append(f"\n## Commands Requiring Confirmation: {', '.join(bash_perms['require_confirmation'])}")

    write_perms = tool_perms.get("write", {})
    if write_perms.get("allowed_extensions"):
        parts.append(f"\n## Allowed File Extensions: {', '.join(write_perms['allowed_extensions'])}")
    if write_perms.get("max_file_size_mb"):
        parts.append(f"\n## Max File Size: {write_perms['max_file_size_mb']}MB")

    return "\n".join(parts) if parts else None


def json_to_options(data: dict | None = None) -> ClaudeAgentOptions:
    """JSONデータからClaudeAgentOptionsを作成する

    Note: コールバック関数(can_use_tool, stderr, hooks)はJSONでシリアライズできないため除外
    """
    if data is None:
        data = {}

    return ClaudeAgentOptions(
        # ツール設定
        tools=data.get("tools"),
        allowed_tools=data.get("allowed_tools", []),
        disallowed_tools=data.get("disallowed_tools", []),

        # プロンプト設定
        system_prompt=data.get("system_prompt"),

        # MCP設定
        mcp_servers=data.get("mcp_servers", {}),

        # 権限設定
        permission_mode=data.get("permission_mode"),
        permission_prompt_tool_name=data.get("permission_prompt_tool_name"),

        # 会話制御
        continue_conversation=data.get("continue_conversation", False),
        resume=data.get("resume"),
        fork_session=data.get("fork_session", False),
        max_turns=data.get("max_turns"),

        # 予算・制限
        max_budget_usd=data.get("max_budget_usd"),
        max_thinking_tokens=data.get("max_thinking_tokens"),
        max_buffer_size=data.get("max_buffer_size"),

        # モデル設定
        model=data.get("model"),
        fallback_model=data.get("fallback_model"),
        betas=data.get("betas", []),

        # パス設定
        cwd=data.get("cwd"),
        cli_path=data.get("cli_path"),
        settings=data.get("settings"),
        add_dirs=data.get("add_dirs", []),

        # 環境設定
        env=data.get("env", {}),
        extra_args=data.get("extra_args", {}),

        # 出力設定
        include_partial_messages=data.get("include_partial_messages", False),
        output_format=data.get("output_format"),

        # エージェント設定
        agents=parse_agents(data.get("agents")),

        # その他
        user=data.get("user"),
        setting_sources=data.get("setting_sources"),
        sandbox=data.get("sandbox"),
        plugins=data.get("plugins", []),
        enable_file_checkpointing=data.get("enable_file_checkpointing", False),
    )

def load_request_from_json(file_path: str | Path) -> dict:
    """JSONファイルからリクエストを読み込む

    JSONファイルの形式:
    {
        "prompt": "プロンプト文字列",
        "options": {
            "cwd": ".",
            "max_turns": 10,
            ...
        }
    }
    """
    path = Path(file_path)
    if not path.exists():
        raise FileNotFoundError(f"JSONファイルが見つかりません: {file_path}")

    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)

    if "prompt" not in data:
        raise ValueError("JSONファイルに'prompt'フィールドが必要です")

    return data


async def request_claude_code(prompt: str, options: ClaudeAgentOptions):
    """Claude Codeにリクエストを送信する"""
    result = []
    async with ClaudeSDKClient(options=options) as client:
        await client.query(prompt)

        async for msg in client.receive_response():
            if isinstance(msg, ResultMessage):
                if msg.subtype == 'success':
                    result.append(msg)
    return result


@mcp.tool()
async def claude_code_query(
    prompt: str,
    options: dict | None = None,
    extra_options: dict | None = None
) -> str:
    """Claude Codeにクエリを送信し、結果をJSON形式で返す

    Args:
        prompt: Claude Codeに送信するプロンプト
        options: ClaudeAgentOptionsの設定（オプション）
        extra_options: 追加オプション（オプション）
            - role: タスクのロール情報。system_promptとして注入される。
                - name: ロール名
                - description: ロールの説明
                - skills: スキルのリスト
                - role_id: ロールID
                - tool_permissions: ツール権限（bash, write）- CLAUDE.md形式の詳細な権限
                - file_permissions: ファイル権限（allowed_paths, denied_paths, read_only_paths）
                - Note: subagentsはoptions.agentsと重複するため使用されない

    Returns:
        結果のJSON文字列
    """
    # optionsからClaudeAgentOptionsを作成
    agent_options = json_to_options(options)

    # extra_options.roleからsystem_promptを生成し、既存のsystem_promptと結合
    if extra_options and extra_options.get("role"):
        role = extra_options["role"]
        role_system_prompt = build_role_system_prompt(role)
        if role_system_prompt:
            if agent_options.system_prompt:
                agent_options.system_prompt = f"{role_system_prompt}\n\n{agent_options.system_prompt}"
            else:
                agent_options.system_prompt = role_system_prompt

    result = await request_claude_code(prompt, agent_options)

    # 結果をJSONに変換
    results_json = [asdict(msg) for msg in result]
    return json.dumps(results_json, ensure_ascii=False, indent=2)


async def request_from_json(file_path: str | Path):
    """JSONファイルからリクエストを読み込んで実行する"""
    data = load_request_from_json(file_path)
    prompt = data["prompt"]
    options = json_to_options(data.get("options"))

    result = await request_claude_code(prompt, options)
    for msg in result:
        print(json.dumps(asdict(msg), ensure_ascii=False, indent=2))

def parse_serve_args(args: list[str]) -> tuple[str, int]:
    """serveコマンドの引数を解析する

    Returns:
        (transport, port): トランスポート種別とポート番号
    """
    transport = "stdio"
    port = 8000

    i = 0
    while i < len(args):
        if args[i] == "--http":
            transport = "streamable-http"
        elif args[i] == "--port" and i + 1 < len(args):
            port = int(args[i + 1])
            i += 1
        i += 1

    return transport, port


async def main():
    if len(sys.argv) < 2:
        print("使用方法:")
        print("  uv run main.py serve                    # stdio MCPサーバーとして起動")
        print("  uv run main.py serve --http             # HTTP MCPサーバーとして起動 (port 8000)")
        print("  uv run main.py serve --http --port 3000 # HTTP MCPサーバーとして起動 (port 3000)")
        print("  uv run main.py <request.json>           # JSONファイルからリクエスト実行")
        print("")
        print("JSONファイルの形式:")
        print(json.dumps({
            "prompt": "プロンプト文字列",
            "options": {
                "cwd": ".",
                "max_turns": 10,
                "permission_mode": "acceptEdits"
            }
        }, indent=2, ensure_ascii=False))
        sys.exit(1)

    if sys.argv[1] == "serve":
        transport, port = parse_serve_args(sys.argv[2:])

        if transport == "stdio":
            # stdioモード（バナーを無効化）
            await mcp.run_async(transport="stdio", show_banner=False)
        else:
            # HTTPモード（streamable-http）
            print(f"Starting HTTP MCP server on port {port}...")
            await mcp.run_async(transport="streamable-http", host="0.0.0.0", port=port)
    else:
        # JSONファイルからリクエスト実行
        json_file = sys.argv[1]
        await request_from_json(json_file)


if __name__ == "__main__":
    anyio.run(main)
