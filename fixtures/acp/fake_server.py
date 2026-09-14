import json
import sys
import time

print("fake-acp banner", flush=True)
turn = 0
sessions = 0
legacy_models = "--legacy-models" in sys.argv
no_model_list = "--no-model-list" in sys.argv
silent_prompt = "--silent-prompt" in sys.argv
late_update = "--late-update" in sys.argv
hang_after_prompt = "--hang-after-prompt" in sys.argv
exit_after_first_turn = "--exit-after-first-turn" in sys.argv
large_turns = "--large-turns" in sys.argv
legacy_cache_usage = "--legacy-cache-usage" in sys.argv
repeat_tool_calls = "--repeat-tool-calls" in sys.argv
other_session_tool_call = "--other-session-tool-call" in sys.argv
untitled_tool_call = "--untitled-tool-call" in sys.argv
fail_prompt_after_tools = "--fail-prompt-after-tools" in sys.argv
start_file = next((arg.split("=", 1)[1] for arg in sys.argv if arg.startswith("--start-file=")), None)
close_file = next((arg.split("=", 1)[1] for arg in sys.argv if arg.startswith("--close-file=")), None)
if start_file:
    with open(start_file, "a", encoding="utf-8") as target:
        target.write("start\n")
config = {"model": "fake-model", "thinking": "high", "mode": "default"}
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    rid = request.get("id")
    if method == "initialize":
        print(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {"agentCapabilities": {"sessionCapabilities": {"close": True}}}}), flush=True)
    elif method == "session/new":
        sessions += 1
        result = {"sessionId": f"fake-session-{sessions}"}
        if not legacy_models and not no_model_list:
            result["configOptions"] = [
                {"id": "model", "category": "model", "currentValue": "fake-model", "options": [{"value": "fake-model"}, {"value": "fake-model/model"}, {"value": "other-model"}]},
                {"id": "thinking", "category": "thought_level", "currentValue": "high", "options": [{"value": "low"}, {"value": "high"}, {"value": "max"}]},
                {"id": "mode", "category": "mode", "currentValue": "default", "options": [{"value": "default"}, {"value": "plan"}]},
            ]
        print(json.dumps({"jsonrpc": "2.0", "id": rid, "result": result}), flush=True)
    elif method == "session/set_config_option":
        if legacy_models:
            print(json.dumps({"jsonrpc": "2.0", "id": rid, "error": {"code": -32601, "message": "legacy server"}}), flush=True)
        else:
            config[request["params"]["configId"]] = request["params"]["value"]
            print(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {}}), flush=True)
    elif method == "session/set_model":
        model = request["params"]["modelId"]
        print(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {"models": {"availableModels": [{"modelId": model}]}}}), flush=True)
    elif method == "session/prompt":
        turn += 1
        sid = request["params"]["sessionId"]
        prompt = request["params"]["prompt"][0]["text"]
        if silent_prompt or hang_after_prompt:
            time.sleep(60)
            continue
        if repeat_tool_calls:
            for _ in range(3):
                print(json.dumps({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid, "update": {"sessionUpdate": "tool_call", "title": "Read file"}}}), flush=True)
        if other_session_tool_call:
            print(json.dumps({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": "other", "update": {"sessionUpdate": "tool_call", "title": "Wrong session"}}}), flush=True)
        if untitled_tool_call:
            print(json.dumps({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid, "update": {"sessionUpdate": "tool_call"}}}), flush=True)
        kind = "read" if prompt == "read" else "execute"
        title = "Read misleading title" if kind == "execute" else "Read file"
        if repeat_tool_calls:
            title = "Read file"
        tool_call = {"kind": kind} if untitled_tool_call else {"kind": kind, "title": title}
        print(json.dumps({"jsonrpc": "2.0", "id": 900 + turn, "method": "session/request_permission", "params": {"sessionId": sid, "toolCall": tool_call, "options": [{"optionId": "always", "kind": "allow_always"}, {"optionId": "once", "kind": "allow_once"}, {"optionId": "no", "kind": "reject_once"}]}}), flush=True)
        permission = json.loads(sys.stdin.readline())
        choice = permission.get("result", {}).get("outcome", {}).get("optionId", "none")
        print(json.dumps({"jsonrpc": "2.0", "id": 800 + turn, "method": "unsupported/test", "params": {}}), flush=True)
        unsupported = json.loads(sys.stdin.readline())
        if unsupported.get("error", {}).get("code") != -32601:
            raise RuntimeError("unknown request response did not use -32601")
        if fail_prompt_after_tools:
            print(json.dumps({"jsonrpc": "2.0", "id": rid, "error": {"code": -32000, "message": "fake prompt failure"}}), flush=True)
            continue
        if late_update and turn == 1:
            print(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {"usage": {"inputTokens": 11, "outputTokens": 7, "cachedReadTokens": 5}}}), flush=True)
            print(json.dumps({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid, "update": {"sessionUpdate": "agent_message_chunk", "content": {"text": "LATE"}}}}), flush=True)
            continue
        print(json.dumps({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": "other", "update": {"sessionUpdate": "agent_message_chunk", "content": {"text": "WRONG"}}}}), flush=True)
        text = f"config:{config['model']}:{config['mode']}:{config['thinking']}" if prompt == "config" else f"turn-{turn}:{choice}"
        if large_turns:
            print(json.dumps({"padding": "x" * (5 * 1024 * 1024)}), flush=True)
        print(json.dumps({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid, "update": {"sessionUpdate": "agent_message_chunk", "content": {"text": text}}}}), flush=True)
        cache_key = "cacheReadTokens" if legacy_cache_usage else "cachedReadTokens"
        print(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {"usage": {"inputTokens": 11, "outputTokens": 7, cache_key: 5}}}), flush=True)
        if exit_after_first_turn and turn == 1:
            sys.exit(0)
    elif method == "session/close":
        if close_file:
            with open(close_file, "w", encoding="utf-8") as target:
                target.write(request["params"]["sessionId"])
        print(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {}}), flush=True)
