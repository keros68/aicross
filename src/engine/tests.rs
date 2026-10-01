//! 引擎单元测试（逐夹具金标准重放在 golden.rs）。

use super::*;

fn temp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("aicross-engine-{name}-{}", Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    p
}

fn turn(id: &str, parents: &[&str], role: &str, created_at: &str) -> GraphNode {
    GraphNode {
        id: id.into(),
        kind: "turn".into(),
        parents: parents.iter().map(|p| p.to_string()).collect(),
        target: Some(Target {
            agent: "codex".into(),
            model: "exec".into(),
            role: role.into(),
            thinking: "中".into(),
            visibility: "共享".into(),
            write: "只读".into(),
            ..Target::default()
        }),
        reply: Some(Reply {
            status: "draft".into(),
            ..Reply::default()
        }),
        created_at: created_at.into(),
        ..GraphNode::default()
    }
}

fn sample_result(node_id: &str, text: &str) -> DispatchNodeResult {
    DispatchNodeResult {
        node_id: node_id.into(),
        stage: 1,
        status: "ok".into(),
        text: text.into(),
        model_reported: "exec".into(),
        tokens_in: 11,
        tokens_out: 22,
        duration_ms: 33,
        exit_code: Some(0),
        isolation: "完全".into(),
        runner: "oneshot".into(),
        ..DispatchNodeResult::default()
    }
}

// ── 保留的老测试 ──────────────────────────────────────────────────────────

#[test]
fn settings_missing_invalid_and_clamped() {
    let root = temp("settings");
    assert_eq!(load_settings(&root), EngineSettings::default());
    fs::write(root.join("dispatch-settings.json"), "{").unwrap();
    assert_eq!(load_settings(&root), EngineSettings::default());
    fs::write(root.join("dispatch-settings.json"),r#"{"timeoutSecs":0,"concurrency":99,"maxRequests":101,"swarmLimit":0,"consoleRunLimit":9,"fixRoundLimit":12.8,"redact":false}"#).unwrap();
    let s = load_settings(&root);
    assert_eq!(
        (
            s.timeout_secs,
            s.concurrency,
            s.max_requests,
            s.swarm_limit,
            s.console_run_limit,
            s.fix_round_limit
        ),
        (1, 3, 100, 1, 3, 9)
    );
    assert!(!s.redact);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn atomic_run_write_leaves_only_target() {
    let root = temp("atomic");
    let mut run = EngineRun {
        run_id: "r1".into(),
        status: "running".into(),
        pid: std::process::id(),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();
    run.status = "done".into();
    write_run(&root, &run).unwrap();
    assert_eq!(fs::read_dir(runs_dir(&root)).unwrap().count(), 1);
    assert_eq!(read_run(&root, "r1").unwrap().status, "done");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dead_pid_becomes_failed() {
    let root = temp("dead");
    let run = EngineRun {
        run_id: "r1".into(),
        status: "running".into(),
        pid: u32::MAX,
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();
    let got = read_run(&root, "r1").unwrap();
    assert_eq!(got.status, "failed");
    assert_eq!(got.reason, "引擎进程已退出");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn utf16_excerpt_covers_chinese_and_emoji() {
    assert_eq!(utf16_excerpt(&"中".repeat(161), 160), "中".repeat(160));
    // 160 码元边界切中 emoji 代理对的一半，孤立半码元按 JS 序列化口径落成 U+FFFD。
    let text = format!("{}😀尾", "a".repeat(159));
    assert_eq!(utf16_excerpt(&text, 160), format!("{}\u{FFFD}", "a".repeat(159)));
}

#[test]
fn adopt_changes_persisted_graph_without_window() {
    let root = temp("adopt");
    let mut graph = new_graph("D:/p", "g1".into(), "root".into(), now());
    graph.nodes.get_mut("root").unwrap().reply = Some(Reply {
        status: "done".into(),
        text: "answer".into(),
        ..Reply::default()
    });
    orchestration::save_graph_in(&root, &graph).unwrap();
    let run = EngineRun {
        run_id: "r1".into(),
        graph_id: "g1".into(),
        project: "D:/p".into(),
        status: "done".into(),
        nodes: node_summary(&graph, vec!["root".into()]),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();
    adopt(&root, "r1", "root").unwrap();
    let loaded = orchestration::load_graph_in(&root, "g1").unwrap().unwrap();
    assert_eq!(
        loaded.nodes["root"].reply.as_ref().unwrap().adopted,
        Some(true)
    );
    let _ = fs::remove_dir_all(root);
}

// ── R1-5：后台进程不继承标准输入输出 ──────────────────────────────────────

/// 中介进程（以 --ignored 手动拉起才会跑）：spawn_detached 一个长跑孙子后
/// 立即退出。孙子的 stdio 若被正确置 null，父进程的管道读端立刻拿到 EOF。
#[cfg(windows)]
#[test]
#[ignore = "只作为 pipe_eof 测试的中介进程运行"]
fn pipe_eof_helper() {
    let ping = Path::new(r"C:\Windows\System32\ping.exe");
    let _grandchild = spawn_detached(ping, &["-n".into(), "20".into(), "127.0.0.1".into()])
        .expect("拉起孙子进程");
}

#[cfg(windows)]
#[test]
fn detached_grandchild_does_not_hold_the_pipe() {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let helper = std::env::current_exe().unwrap();
    let mut child = Command::new(helper)
        .args([
            "--ignored",
            "--exact",
            "engine::tests::pipe_eof_helper",
            "--nocapture",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("拉起中介进程");
    let mut stdout = child.stdout.take().unwrap();
    let started = std::time::Instant::now();
    let mut sink = vec![];
    stdout.read_to_end(&mut sink).expect("读中介输出");
    let elapsed = started.elapsed();
    let _ = child.wait();
    // 中介立即退出；孙子（ping -n 20，约 19 秒）若继承了管道写端，EOF 会被拖住。
    assert!(
        elapsed < Duration::from_secs(10),
        "管道 EOF 等了 {elapsed:?}，后台进程继承了标准输出"
    );
}

#[cfg(not(windows))]
#[test]
fn spawn_detached_detaches_stdio() {
    let pid = spawn_detached(Path::new("/bin/true"), &[]).expect("拉起 /bin/true");
    assert!(pid > 0);
}

#[test]
fn spawn_detached_missing_exe_is_error() {
    let missing = Path::new("D:/nonexistent/aicross-no-such.exe");
    assert!(spawn_detached(missing, &[]).is_err());
}

// ── R1-6：run 文件写竞态 ──────────────────────────────────────────────────

#[test]
fn pid_zero_within_startup_window_stays_running() {
    let root = temp("pid0-live");
    let run = EngineRun {
        run_id: "r1".into(),
        status: "running".into(),
        pid: 0,
        updated_at: now(),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();
    let before = fs::read(run_path(&root, "r1")).unwrap();
    let got = read_run(&root, "r1").unwrap();
    assert_eq!(got.status, "running");
    assert_eq!(fs::read(run_path(&root, "r1")).unwrap(), before, "启动中的 run 不应被改写");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pid_zero_past_startup_window_becomes_failed() {
    let root = temp("pid0-dead");
    let run = EngineRun {
        run_id: "r1".into(),
        status: "running".into(),
        pid: 0,
        updated_at: rfc3339(Utc::now() - chrono::Duration::seconds(STARTUP_WINDOW_SECS + 5)),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();
    let got = read_run(&root, "r1").unwrap();
    assert_eq!(got.status, "failed");
    assert_eq!(got.reason, "引擎进程未能启动");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cancel_non_running_run_is_silent_ok_without_marker() {
    let root = temp("cancel-done");
    let run = EngineRun {
        run_id: "r1".into(),
        status: "done".into(),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();
    let got = cancel(&root, "r1").unwrap();
    assert_eq!(got.status, "done");
    assert!(!cancel_path(&root, "r1").exists(), "不在 running 不写取消标记");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn adopt_node_without_reply_leaves_graph_untouched() {
    let root = temp("adopt-noreply");
    let graph = new_graph("D:/p", "g1".into(), "root".into(), now());
    orchestration::save_graph_in(&root, &graph).unwrap();
    let before = serde_json::to_value(&graph).unwrap();
    let run = EngineRun {
        run_id: "r1".into(),
        graph_id: "g1".into(),
        project: "D:/p".into(),
        status: "done".into(),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();
    adopt(&root, "r1", "root").unwrap();
    let loaded = orchestration::load_graph_in(&root, "g1").unwrap().unwrap();
    assert_eq!(serde_json::to_value(&loaded).unwrap(), before, "无结果的节点不改图");
    let _ = fs::remove_dir_all(root);
}

// ── R1-7：Windows 下 pid 复用核对 ─────────────────────────────────────────

#[cfg(windows)]
#[test]
fn pid_reuse_guard_checks_creation_time() {
    let root = temp("pid-reuse");
    let run = EngineRun {
        run_id: "r1".into(),
        status: "running".into(),
        pid: std::process::id(),
        pid_started_at: own_process_started_at(),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();
    assert_eq!(read_run(&root, "r1").unwrap().status, "running");
    let mut stale = read_run(&root, "r1").unwrap();
    stale.pid_started_at = "1".into();
    stale.status = "running".into();
    write_run(&root, &stale).unwrap();
    let got = read_run(&root, "r1").unwrap();
    assert_eq!(got.status, "failed");
    assert_eq!(got.reason, "引擎进程已退出");
    let _ = fs::remove_dir_all(root);
}

// ── R1-9：latest_leaf 跳过遗留节点、同深取较新 ────────────────────────────

#[test]
fn latest_leaf_skips_leftover_and_prefers_newer_at_same_depth() {
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), "2026-01-01T00:00:00.000Z".into());
    for n in [
        turn("a", &["root"], "执行", "2026-01-01T00:00:01.000Z"),
        turn("b", &["root"], "执行", "2026-01-01T00:00:02.000Z"),
    ] {
        graph.nodes.insert(n.id.clone(), n);
    }
    // 最深的节点带 leftover → 不算列末端。
    let mut leftover_node = turn("lo", &["b"], "验收", "2026-01-01T00:00:03.000Z");
    leftover_node.leftover = Some(Leftover {
        source_node_id: "b".into(),
        items: vec!["- 未闭合".into()],
    });
    graph.nodes.insert("lo".into(), leftover_node);
    assert_eq!(latest_leaf(&graph), "b");
    // 同深同创建时间：先插入的留下（严格较新才替换）。
    let mut graph2 = graph.clone();
    graph2.nodes.insert(
        "c".into(),
        turn("c", &["root"], "执行", "2026-01-01T00:00:02.000Z"),
    );
    assert_eq!(latest_leaf(&graph2), "b");
}

// ── R1-11：close_waiting_acceptance 连验证兄弟一并标跳过 ──────────────────

#[test]
fn close_waiting_acceptance_marks_verification_siblings() {
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), "2026-01-01T00:00:00.000Z".into());
    let mut exec = turn("exec", &["root"], "执行", "2026-01-01T00:00:01.000Z");
    exec.reply = Some(Reply {
        status: "failed".into(),
        error_kind: Some("validation".into()),
        ..Reply::default()
    });
    graph.nodes.insert("exec".into(), exec);
    let accept = turn("acc", &["exec"], "验收", "2026-01-01T00:00:02.000Z");
    let verify = turn("ver", &["exec"], "验证", "2026-01-01T00:00:03.000Z");
    graph.nodes.insert("acc".into(), accept);
    graph.nodes.insert("ver".into(), verify);
    let (next, changed) = close_waiting_acceptance(&graph, "exec", "上游验证未通过");
    assert!(changed);
    for id in ["acc", "ver"] {
        let reply = next.nodes[id].reply.as_ref().unwrap();
        assert_eq!(reply.status, "draft");
        assert_eq!(reply.skipped_reason.as_deref(), Some("上游验证未通过"));
    }
    // 标过跳过的节点状态仍是 draft（TS 口径），重复调用覆写同样的值。
    let (again, _) = close_waiting_acceptance(&next, "exec", "上游验证未通过");
    assert_eq!(serde_json::to_value(&again).unwrap(), serde_json::to_value(&next).unwrap());
}

// ── R1-15：遗留条目识别 ───────────────────────────────────────────────────

#[test]
fn leftover_items_recognize_bullet_forms() {
    let text = "NEEDS_FIX\n3 处需要修改\n（1）补入口校验\n(2) 补错误处理\n- 更新文档\n* 补注释\n• 收尾\n1. 第一项\n2、第二项\n3) 第三项";
    assert_eq!(
        leftover_items(text),
        vec![
            "（1）补入口校验",
            "(2) 补错误处理",
            "- 更新文档",
            "* 补注释",
            "• 收尾",
            "1. 第一项",
            "2、第二项",
            "3) 第三项",
        ]
    );
    // 没有条目行 → 整段按行兜底。
    assert_eq!(
        leftover_items("NEEDS_FIX\n3 处需要修改\n入口校验缺失"),
        vec!["NEEDS_FIX", "3 处需要修改", "入口校验缺失"]
    );
}

// ── R1-18：continuation 的旧图标记与 provider 缺省 ────────────────────────

#[test]
fn continuation_breaks_on_fallback_notice_and_tolerates_missing_provider() {
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), now());
    let mut parent = turn("p", &["root"], "执行", "2026-01-01T00:00:01.000Z");
    parent.target.as_mut().unwrap().agent = "claude".into();
    parent.target.as_mut().unwrap().model = "plan".into();
    parent.reply = Some(Reply {
        status: "done".into(),
        text: "第一棒".into(),
        session_id: Some("sess-1".into()),
        runner: Some("oneshot".into()),
        ..Reply::default()
    });
    let mut child = turn("c", &["p"], "执行", "2026-01-01T00:00:02.000Z");
    child.target.as_mut().unwrap().agent = "claude".into();
    child.target.as_mut().unwrap().model = "plan".into();
    // provider 一个缺省一个空串：视为相等，仍然续会话。
    parent.target.as_mut().unwrap().provider = None;
    child.target.as_mut().unwrap().provider = Some(String::new());
    graph.nodes.insert("p".into(), parent.clone());
    graph.nodes.insert("c".into(), child.clone());
    assert_eq!(continuation(&graph, "c"), ("sess-1".to_string(), "oneshot".to_string()));
    // 备选作答（字段）：不续。
    let mut with_fallback = graph.clone();
    with_fallback.nodes.get_mut("p").unwrap().reply.as_mut().unwrap().fallback_from =
        Some("codex/exec".into());
    assert_eq!(continuation(&with_fallback, "c"), (String::new(), String::new()));
    // 旧图只有正文首行标记：也不续。
    let mut old_style = graph.clone();
    let reply = old_style.nodes.get_mut("p").unwrap().reply.as_mut().unwrap();
    reply.text = "（已切备选：codex/exec → claude/plan）\n正文".into();
    assert_eq!(continuation(&old_style, "c"), (String::new(), String::new()));
}

// ── R1-19：append_leftover 复用同源节点；append_correction 无产物不生成 ────

#[test]
fn append_leftover_reuses_same_source_node() {
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), now());
    let mut accept = turn("acc", &["root"], "验收", "2026-01-01T00:00:01.000Z");
    accept.reply = Some(Reply {
        status: "done".into(),
        text: "NEEDS_FIX\n- 旧账未清".into(),
        ..Reply::default()
    });
    graph.nodes.insert("acc".into(), accept);
    let mut seq = 0usize;
    let mut ids = move || {
        seq += 1;
        format!("gen-{seq}")
    };
    let (with_one, first) = append_leftover(&graph, "acc", &mut ids, now());
    assert_eq!(first.as_deref(), Some("gen-1"));
    // 再来一次：复用 gen-1 覆写，不消耗新 id、不多出节点。
    let (with_two, second) = append_leftover(&with_one, "acc", &mut ids, now());
    assert_eq!(second.as_deref(), Some("gen-1"));
    assert_eq!(with_two.nodes.len(), with_one.nodes.len());
    assert_eq!(
        with_two.nodes["gen-1"].leftover.as_ref().unwrap().items,
        vec!["- 旧账未清"]
    );
}

#[test]
fn append_correction_skips_when_first_execution_parent_has_no_artifacts() {
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), now());
    let exec = turn("exec", &["root"], "执行", "2026-01-01T00:00:01.000Z");
    graph.nodes.insert("exec".into(), exec);
    let mut accept = turn("acc", &["exec"], "验收", "2026-01-01T00:00:02.000Z");
    accept.reply = Some(Reply {
        status: "done".into(),
        text: "NEEDS_FIX\n- 补测试".into(),
        ..Reply::default()
    });
    graph.nodes.insert("acc".into(), accept);
    let mut ids = || "gen-1".to_string();
    let (next, made) = append_correction(&graph, "acc", &mut ids, Utc::now());
    assert!(made.is_empty(), "第一个执行父没有产物则不生成修正轮");
    assert_eq!(next.nodes.len(), graph.nodes.len());
}

// ── R1-20：upstream_artifacts 的不可达兜底 ────────────────────────────────

#[test]
fn upstream_artifacts_falls_back_for_unreachable_allowed_nodes() {
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), now());
    // 挂在另一支上的执行节点（不是 node 的祖先），带产物。
    let mut other = turn("other", &["root"], "执行", "2026-01-01T00:00:01.000Z");
    other.reply = Some(Reply {
        status: "done".into(),
        artifacts: Some(ReplyArtifacts {
            diff: "diff --git a/a b/a".into(),
            worktree: Some("D:/p/.aicross/worktrees/x".into()),
            stdout_summary: "通过".into(),
            ..ReplyArtifacts::default()
        }),
        ..Reply::default()
    });
    graph.nodes.insert("other".into(), other);
    let mut blind = turn("b", &["root"], "验证", "2026-01-01T00:00:02.000Z");
    let t = blind.target.as_mut().unwrap();
    t.visibility = "指定上游".into();
    t.upstreams = Some(vec!["other".into()]);
    graph.nodes.insert("b".into(), blind);
    let artifacts = upstream_artifacts(&graph, "b");
    assert_eq!(artifacts.len(), 1, "指定上游不在祖先链上也要拿到产物（兜底）");
    assert_eq!(artifacts[0].stdout_summary, "通过");
}

// ── R1-21：parse_acceptance 的 UTF-16 尾窗 ────────────────────────────────

#[test]
fn parse_acceptance_uses_utf16_tail_window() {
    // PASS 在 1200 码元窗口之外（700 个 emoji = 1400 码元）→ 判 UNKNOWN。
    let far = format!("PASS{}", "😀".repeat(700));
    assert_eq!(parse_acceptance(&far), "UNKNOWN");
    // 窗口内取最后一次出现。
    assert_eq!(parse_acceptance("NEEDS_FIX\n后改口 PASS"), "PASS");
    assert_eq!(parse_acceptance("先说 PASS\n核对后 NEEDS_FIX"), "NEEDS_FIX");
    assert_eq!(parse_acceptance("PASSPORT 不算"), "UNKNOWN");
}

// ── R1-24：active_runs 只数 .json 且不以点开头 ────────────────────────────

#[test]
fn active_runs_counts_only_json_run_files() {
    let root = temp("active");
    let running = EngineRun {
        run_id: "a".into(),
        status: "running".into(),
        pid: 0,
        updated_at: now(),
        ..EngineRun::default()
    };
    write_run(&root, &running).unwrap();
    let done = EngineRun {
        run_id: "b".into(),
        status: "done".into(),
        ..EngineRun::default()
    };
    write_run(&root, &done).unwrap();
    let dir = runs_dir(&root);
    fs::write(dir.join(".hidden.json"), "{}").unwrap();
    fs::write(dir.join("notes.txt"), "running").unwrap();
    fs::write(cancel_path(&root, "a"), []).unwrap();
    fs::write(dir.join("broken.json"), "{").unwrap();
    assert_eq!(active_runs(&root), 1);
    let _ = fs::remove_dir_all(root);
}

// ── R1-26：路径归一化 ─────────────────────────────────────────────────────

#[test]
fn normalized_strips_extended_prefixes() {
    assert_eq!(normalized(r"D:\Work\Project\"), "d:/work/project");
    assert_eq!(normalized(r"\\?\D:\Work"), "d:/work");
    assert_eq!(normalized(r"\\?\UNC\server\share\dir"), "//server/share/dir");
    assert_eq!(normalized(r"\\?\uNc\SERVER\share"), "//server/share");
    assert_eq!(normalized("d:/work//"), "d:/work");
}

// ── R1-29：取消 watcher 与取消收尾 ────────────────────────────────────────

#[test]
fn watch_cancel_marks_flag_and_cancels_in_flight_nodes() {
    let root = temp("watch");
    let run_id = format!("watch-{}", Uuid::new_v4());
    let active = Arc::new(Mutex::new(vec!["node-x".to_string()]));
    let cancelled = Arc::new(AtomicBool::new(false));
    let handle = watch_cancel(
        root.clone(),
        run_id.clone(),
        active,
        Arc::clone(&cancelled),
    );
    fs::create_dir_all(runs_dir(&root)).unwrap();
    fs::write(cancel_path(&root, &run_id), []).unwrap();
    for _ in 0..50 {
        if cancelled.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    handle.join().expect("watcher 退出");
    assert!(cancelled.load(Ordering::SeqCst));
    assert!(
        dispatch::node_is_cancelled(&run_id, "node-x"),
        "在飞节点应收到取消"
    );
    let _ = fs::remove_dir_all(root);
}

struct StubSink {
    updates: Vec<RunUpdate>,
    results: Vec<DispatchNodeResult>,
}

impl PipelineSink for StubSink {
    fn save_graph(&mut self, _graph: &Graph) -> Result<Option<Graph>, String> {
        Ok(None)
    }
    fn update(&mut self, update: &RunUpdate) -> Result<(), String> {
        self.updates.push(update.clone());
        Ok(())
    }
    fn dispatch(&mut self, request: &DispatchStageRequest) -> Result<DispatchStageResult, String> {
        Ok(DispatchStageResult {
            run_id: request.run_id.clone(),
            results: self.results.clone(),
            state_file: String::new(),
        })
    }
}

#[test]
fn cancelled_pipeline_marks_in_flight_failed_and_ends_run_failed() {
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), now());
    graph.nodes.insert("n1".into(), turn("n1", &["root"], "执行", &now()));
    let settings = EngineSettings::default();
    let mut sink = StubSink {
        updates: vec![],
        results: vec![],
    };
    let calls = std::cell::Cell::new(0usize);
    // 检查点：循环顶（1）、登记在飞后（2）、结果写回图后（3）。本阶段没返回
    // 任何结果，在飞节点由收尾统一标「派发已取消」。
    let is_cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() >= 3
    };
    let mut ids = || Uuid::new_v4().to_string();
    let mut clock = Utc::now;
    let outcome = run_pipeline(
        "run-c",
        "D:/p",
        &settings,
        graph,
        &["n1".to_string()],
        &[],
        "原始需求",
        &mut sink,
        &mut ids,
        &mut clock,
        &is_cancelled,
    )
    .expect("取消不是错误");
    assert_eq!(outcome.error, CANCELLED_TEXT);
    let node = &outcome.graph.nodes["n1"];
    let reply = node.reply.as_ref().unwrap();
    assert_eq!(reply.status, "failed");
    assert_eq!(reply.error_kind.as_deref(), Some("cancelled"));
    assert_eq!(reply.text, CANCELLED_TEXT);
    let terminal = sink.updates.last().unwrap();
    assert_eq!(terminal.status, "failed");
    assert_eq!(terminal.reason, CANCELLED_TEXT);
}

#[test]
fn cancel_after_results_writes_them_back_and_marks_rest_cancelled() {
    // R2-8：判定取消时，本阶段已返回的结果先写回图，没返回的才标「派发已取消」。
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), now());
    graph.nodes.insert("n1".into(), turn("n1", &["root"], "执行", &now()));
    graph.nodes.insert("n2".into(), turn("n2", &["root"], "执行", &now()));
    let settings = EngineSettings::default();
    let mut sink = StubSink {
        updates: vec![],
        // 只返回 n1 的结果，n2 仍在飞。
        results: vec![sample_result("n1", "完成")],
    };
    let calls = std::cell::Cell::new(0usize);
    let is_cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() >= 3
    };
    let mut ids = || Uuid::new_v4().to_string();
    let mut clock = Utc::now;
    let outcome = run_pipeline(
        "run-c",
        "D:/p",
        &settings,
        graph,
        &["n1".to_string(), "n2".to_string()],
        &[],
        "原始需求",
        &mut sink,
        &mut ids,
        &mut clock,
        &is_cancelled,
    )
    .expect("取消不是错误");
    assert_eq!(outcome.error, CANCELLED_TEXT);
    let n1 = outcome.graph.nodes["n1"].reply.as_ref().unwrap();
    assert_eq!(n1.status, "done", "已返回的结果必须写回图");
    assert_eq!(n1.text, "完成");
    let n2 = outcome.graph.nodes["n2"].reply.as_ref().unwrap();
    assert_eq!(n2.status, "failed");
    assert_eq!(n2.error_kind.as_deref(), Some("cancelled"));
    assert_eq!(n2.text, CANCELLED_TEXT);
    let terminal = sink.updates.last().unwrap();
    assert_eq!(terminal.status, "failed");
    assert_eq!(terminal.reason, CANCELLED_TEXT);
}

#[test]
fn cancel_right_after_registration_skips_dispatch_and_keeps_draft() {
    // R2-8：取消标记落在循环顶检查与登记在飞之间时，不再把这一阶段跑完。
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), now());
    graph.nodes.insert("n1".into(), turn("n1", &["root"], "执行", &now()));
    let settings = EngineSettings::default();
    let mut sink = StubSink {
        updates: vec![],
        results: vec![sample_result("n1", "完成")],
    };
    let calls = std::cell::Cell::new(0usize);
    let is_cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() >= 2
    };
    let mut ids = || Uuid::new_v4().to_string();
    let mut clock = Utc::now;
    let outcome = run_pipeline(
        "run-c",
        "D:/p",
        &settings,
        graph,
        &["n1".to_string()],
        &[],
        "原始需求",
        &mut sink,
        &mut ids,
        &mut clock,
        &is_cancelled,
    )
    .expect("取消不是错误");
    assert_eq!(outcome.error, CANCELLED_TEXT);
    assert_eq!(
        outcome.graph.nodes["n1"].reply.as_ref().unwrap().status,
        "draft",
        "未派出就取消的节点保持 draft"
    );
    assert_eq!(sink.updates.last().unwrap().status, "failed");
}

// ── R2-1：含多字节字符的项目路径不 panic ──────────────────────────

#[test]
fn normalized_handles_multibyte_paths() {
    // 旧实现按字节切片 value[..8]，这些路径会切在字符中间而 panic。
    assert_eq!(normalized(r"D:\项目"), "d:/项目");
    assert_eq!(normalized(r"D:\项"), "d:/项");
    assert_eq!(normalized(r"\\?\D:\论文\数据"), "d:/论文/数据");
    assert_eq!(normalized(r"\\?\UNC\服务器\共享\目录"), "//服务器/共享/目录");
    // 与 TS normalizedPath 口径一致：只剥真正的前缀，剥不到就按原值归一。
    assert_eq!(normalized(r"项\目"), "项/目");
}

#[test]
fn plan_dispatch_works_with_chinese_project_graph_in_dir() {
    let root = temp("cn-graph-dir");
    let dir = root.join("graphs");
    fs::create_dir_all(&dir).unwrap();
    // 图目录里已有一张中文路径的图：pick_project_graph 对每张图都做路径归一，
    // 不能因此 panic。
    let chinese = new_graph(r"D:\项目", "g-cn".into(), "root".into(), now());
    orchestration::save_graph_in(&root, &chinese).unwrap();
    assert_eq!(latest_project_graph(&root, r"D:\项目").map(|g| g.id), Some("g-cn".into()));
    // 对另一个项目的 plan_dispatch 照常工作。
    let existing = latest_project_graph(&root, r"D:\别的");
    assert!(existing.is_none());
    let models = vec![ResourceModel {
        agent: "codex".into(),
        id: "exec".into(),
        ..ResourceModel::default()
    }];
    let presets = orchestration::builtin_presets();
    let roles = RoleSettings::default();
    let settings = EngineSettings::default();
    let targets = vec![Target {
        agent: "codex".into(),
        model: "exec".into(),
        role: "执行".into(),
        thinking: "中".into(),
        visibility: "共享".into(),
        write: "只读".into(),
        ..Target::default()
    }];
    let mut git_dirty_count = || None;
    let mut plan_input = PlanInput {
        run_id: "r1",
        project: r"D:\别的",
        text: "实现任务",
        preset: None,
        attachments: &[],
        targets: &targets,
        settings: &settings,
        models: &models,
        resources_error: None,
        presets: &presets,
        roles: &roles,
        active_runs: 0,
        existing,
        git_dirty_count: &mut git_dirty_count,
    };
    let mut seq = 0usize;
    let mut ids = move || {
        seq += 1;
        format!("gen-{seq}")
    };
    let mut clock = || Utc::now();
    match plan_dispatch(&mut plan_input, &mut ids, &mut clock) {
        Plan::Ready { created, .. } => assert!(!created.is_empty()),
        Plan::Rejected { reason, .. } => panic!("plan_dispatch 不应拒绝：{reason}"),
    }
    let _ = fs::remove_dir_all(root);
}

// ── R2-2：合并写：并发 run 与 adopt 的改动不被整图回写抹掉 ──────────

fn file_sink<'a>(root: &'a Path, run: &'a mut EngineRun) -> FileSink<'a> {
    FileSink {
        root,
        run,
        active: Arc::new(Mutex::new(Vec::new())),
        committed: None,
    }
}

#[test]
fn merging_commits_keep_both_runs_nodes_and_results() {
    let root = temp("merge-two-runs");
    let base = new_graph("D:/p", "g1".into(), "root".into(), now());
    orchestration::save_graph_in(&root, &base).unwrap();
    let mut run_a = EngineRun { run_id: "a".into(), graph_id: "g1".into(), ..EngineRun::default() };
    let mut run_b = EngineRun { run_id: "b".into(), graph_id: "g1".into(), ..EngineRun::default() };
    let mut sink_a = file_sink(&root, &mut run_a);
    let mut sink_b = file_sink(&root, &mut run_b);

    // 两个 run 交替提交同一张图：各自新增自己的节点。
    let mut ga = base.clone();
    ga.nodes.insert("a1".into(), turn("a1", &["root"], "执行", &now()));
    sink_a.save_graph(&ga).unwrap();
    let mut gb = base.clone();
    gb.nodes.insert("b1".into(), turn("b1", &["root"], "执行", &now()));
    sink_b.save_graph(&gb).unwrap();

    // a 再提交：a1 出结果；b1 不在本 run 的改动集里，不能被抹掉。
    let mut ga2 = ga.clone();
    ga2.nodes.get_mut("a1").unwrap().reply = Some(Reply {
        status: "done".into(),
        text: "a 结果".into(),
        ..Reply::default()
    });
    ga2.updated_at = now();
    sink_a.save_graph(&ga2).unwrap();
    let disk = orchestration::load_graph_in(&root, "g1").unwrap().unwrap();
    assert_eq!(disk.nodes["a1"].reply.as_ref().unwrap().text, "a 结果");
    assert!(disk.nodes.contains_key("b1"), "另一个 run 的节点不能被抹掉");

    // b 再提交自己的结果，a 的结果也保留。
    let mut gb2 = gb.clone();
    gb2.nodes.get_mut("b1").unwrap().reply = Some(Reply {
        status: "done".into(),
        text: "b 结果".into(),
        ..Reply::default()
    });
    gb2.updated_at = now();
    sink_b.save_graph(&gb2).unwrap();
    let disk = orchestration::load_graph_in(&root, "g1").unwrap().unwrap();
    assert_eq!(disk.nodes["a1"].reply.as_ref().unwrap().text, "a 结果");
    assert_eq!(disk.nodes["b1"].reply.as_ref().unwrap().text, "b 结果");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn adopt_during_run_survives_later_commits() {
    let root = temp("merge-adopt");
    let mut graph = new_graph("D:/p", "g1".into(), "root".into(), now());
    let mut n1 = turn("n1", &["root"], "执行", &now());
    n1.reply = Some(Reply { status: "done".into(), text: "回答".into(), ..Reply::default() });
    graph.nodes.insert("n1".into(), n1);
    graph.nodes.insert("n2".into(), turn("n2", &["root"], "执行", &now()));
    orchestration::save_graph_in(&root, &graph).unwrap();
    let run = EngineRun {
        run_id: "r1".into(),
        graph_id: "g1".into(),
        project: "D:/p".into(),
        status: "running".into(),
        nodes: node_summary(&graph, vec!["n1".into(), "n2".into()]),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();

    // run 进场首次提交（此时 n1 尚未被采纳）。
    let mut run_sink = EngineRun { ..run };
    let mut sink = file_sink(&root, &mut run_sink);
    sink.save_graph(&graph).unwrap();
    // run 进行中用户采纳了 n1。
    adopt(&root, "r1", "n1").unwrap();
    assert_eq!(
        orchestration::load_graph_in(&root, "g1").unwrap().unwrap().nodes["n1"]
            .reply
            .as_ref()
            .unwrap()
            .adopted,
        Some(true)
    );

    // 后台随后提交 n2 的结果：它自己的内存快照里 n1 没有采纳标记，合并写
    // 只覆盖 n2，不能把 n1 的采纳抹掉。
    let mut next = graph.clone();
    next.nodes.get_mut("n2").unwrap().reply = Some(Reply {
        status: "done".into(),
        text: "n2 结果".into(),
        ..Reply::default()
    });
    next.updated_at = now();
    sink.save_graph(&next).unwrap();

    let disk = orchestration::load_graph_in(&root, "g1").unwrap().unwrap();
    assert_eq!(disk.nodes["n1"].reply.as_ref().unwrap().adopted, Some(true), "采纳标记不能被后台提交抹掉");
    assert_eq!(disk.nodes["n2"].reply.as_ref().unwrap().text, "n2 结果");
    let _ = fs::remove_dir_all(root);
}

// ── R2-3：adopt 不写 run 文件 ─────────────────────────────────

#[test]
fn adopt_keeps_run_file_untouched() {
    let root = temp("adopt-run-file");
    let mut graph = new_graph("D:/p", "g1".into(), "root".into(), now());
    let mut n1 = turn("n1", &["root"], "执行", &now());
    n1.reply = Some(Reply { status: "done".into(), text: "回答".into(), ..Reply::default() });
    graph.nodes.insert("n1".into(), n1);
    orchestration::save_graph_in(&root, &graph).unwrap();
    let run = EngineRun {
        run_id: "r1".into(),
        graph_id: "g1".into(),
        project: "D:/p".into(),
        status: "done".into(),
        nodes: node_summary(&graph, vec!["n1".into()]),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();
    let before = fs::read(run_path(&root, "r1")).unwrap();
    adopt(&root, "r1", "n1").unwrap();
    assert_eq!(fs::read(run_path(&root, "r1")).unwrap(), before, "adopt 不改写 run 文件");
    // 图上的采纳标记已经落盘。
    assert_eq!(
        orchestration::load_graph_in(&root, "g1").unwrap().unwrap().nodes["n1"]
            .reply
            .as_ref()
            .unwrap()
            .adopted,
        Some(true)
    );
    let _ = fs::remove_dir_all(root);
}

// ── R2-4：后台进程不复活已判失败的 run ──────────────────────

#[test]
fn run_background_ignores_non_running_run() {
    let root = temp("bg-failed");
    let mut graph = new_graph("D:/p", "g1".into(), "root".into(), now());
    graph.nodes.insert("n1".into(), turn("n1", &["root"], "执行", &now()));
    orchestration::save_graph_in(&root, &graph).unwrap();
    let run = EngineRun {
        run_id: "r1".into(),
        graph_id: "g1".into(),
        project: "D:/p".into(),
        status: "failed".into(),
        reason: "引擎进程未能启动".into(),
        nodes: node_summary(&graph, vec!["n1".into()]),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();
    let run_before = fs::read(run_path(&root, "r1")).unwrap();
    let graph_before = fs::read(root.join("graphs").join("g1.json")).unwrap();
    run_background(&root, "r1").expect("不派发直接成功返回");
    assert_eq!(fs::read(run_path(&root, "r1")).unwrap(), run_before, "run 文件不能被改写");
    assert_eq!(fs::read(root.join("graphs").join("g1.json")).unwrap(), graph_before, "图不能被动");
    let _ = fs::remove_dir_all(root);
}

// ── R2-6：推进中途失败的收尾只动本 run 的节点 ───────────────────

struct FailingCommitSink;

impl PipelineSink for FailingCommitSink {
    fn save_graph(&mut self, _graph: &Graph) -> Result<Option<Graph>, String> {
        Err("写入图失败：模拟磁盘满".into())
    }
    fn update(&mut self, _update: &RunUpdate) -> Result<(), String> {
        Ok(())
    }
    fn dispatch(&mut self, _request: &DispatchStageRequest) -> Result<DispatchStageResult, String> {
        panic!("提交失败后不应再派发")
    }
}

#[test]
fn pipeline_commit_failure_only_fails_this_runs_nodes() {
    let root = temp("commit-fail");
    // 图上：本 run 的 n1 pending、另一个 run 的 n2 pending。
    let mut graph = new_graph("D:/p", "g1".into(), "root".into(), now());
    let mut n1 = turn("n1", &["root"], "执行", &now());
    n1.reply = Some(Reply { status: "pending".into(), ..Reply::default() });
    let mut n2 = turn("n2", &["root"], "执行", &now());
    n2.reply = Some(Reply { status: "pending".into(), ..Reply::default() });
    graph.nodes.insert("n1".into(), n1);
    graph.nodes.insert("n2".into(), n2);
    orchestration::save_graph_in(&root, &graph).unwrap();
    fs::write(
        root.join("resources.json"),
        r#"{"version":1,"agents":[{"id":"codex","models":[{"id":"exec","agent":"codex"}]}]}"#,
    )
    .unwrap();
    let run = EngineRun {
        run_id: "r1".into(),
        graph_id: "g1".into(),
        project: "D:/p".into(),
        status: "running".into(),
        nodes: node_summary(&graph, vec!["n1".into()]),
        ..EngineRun::default()
    };
    write_run(&root, &run).unwrap();

    let result = run_background_with(&root, "r1", |_, _, _, _| Box::new(FailingCommitSink));
    let error = result.expect_err("首次提交就失败");
    assert!(error.contains("写入图失败"));

    let disk = orchestration::load_graph_in(&root, "g1").unwrap().unwrap();
    let n1 = disk.nodes["n1"].reply.as_ref().unwrap();
    assert_eq!(n1.status, "failed");
    assert_eq!(n1.error_kind.as_deref(), Some("error"));
    let n2 = disk.nodes["n2"].reply.as_ref().unwrap();
    assert_eq!(n2.status, "pending", "别的 run 的 pending 节点不能被误标失败");

    let finished = read_run(&root, "r1").unwrap();
    assert_eq!(finished.status, "failed");
    assert!(finished.reason.contains("写入图失败"));
    assert_eq!(finished.nodes.len(), 1);
    assert_eq!(finished.nodes[0].node_id, "n1");
    assert_eq!(finished.nodes[0].status, "failed");
    let _ = fs::remove_dir_all(root);
}

// ── R2-9：upstream_artifacts 第一轮只在允许集内 DFS ─────────────────

#[test]
fn upstream_artifacts_first_pass_dfs_stays_inside_allowed_set() {
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), now());
    let artifact = |summary: &str| {
        Some(ReplyArtifacts {
            stdout_summary: summary.into(),
            ..ReplyArtifacts::default()
        })
    };
    let mut a = turn("a", &["root"], "执行", &now());
    a.reply = Some(Reply {
        status: "done".into(),
        text: "a 回答".into(),
        artifacts: artifact("a 产物"),
        ..Reply::default()
    });
    let mut b = turn("b", &["root"], "执行", &now());
    b.reply = Some(Reply {
        status: "done".into(),
        text: "b 回答".into(),
        artifacts: artifact("b 产物"),
        ..Reply::default()
    });
    graph.nodes.insert("a".into(), a);
    graph.nodes.insert("b".into(), b);
    // 指定上游按 [b, a] 给：root 不在允许集里，整棵剪枝，两路产物都由兑底
    // 按允许集顺序补——不是全祖先 DFS 序（a、b）。
    let mut m = turn("m", &["a", "b"], "验收", &now());
    let t = m.target.as_mut().unwrap();
    t.visibility = "指定上游".into();
    t.upstreams = Some(vec!["b".into(), "a".into()]);
    graph.nodes.insert("m".into(), m);
    let artifacts = upstream_artifacts(&graph, "m");
    let summaries: Vec<_> = artifacts.iter().map(|a| a.stdout_summary.as_str()).collect();
    assert_eq!(summaries, vec!["b 产物", "a 产物"]);
}

// ── R2-10：启动窗口对远未来时间戳也过期 ──────────────────────

#[test]
fn pid_zero_far_future_updated_at_is_expired() {
    let root = temp("pid0-future");
    let far = EngineRun {
        run_id: "far".into(),
        status: "running".into(),
        pid: 0,
        updated_at: rfc3339(Utc::now() + chrono::Duration::seconds(120)),
        ..EngineRun::default()
    };
    write_run(&root, &far).unwrap();
    let got = read_run(&root, "far").unwrap();
    assert_eq!(got.status, "failed");
    assert_eq!(got.reason, "引擎进程未能启动");
    // 小幅未来（时钟微偏）仍算启动中。
    let near = EngineRun {
        run_id: "near".into(),
        status: "running".into(),
        pid: 0,
        updated_at: rfc3339(Utc::now() + chrono::Duration::seconds(5)),
        ..EngineRun::default()
    };
    write_run(&root, &near).unwrap();
    assert_eq!(read_run(&root, "near").unwrap().status, "running");
    let _ = fs::remove_dir_all(root);
}

// ── R2-12：退出码 3 的另两种来源 ────────────────────────────

#[test]
fn start_dispatch_reports_unavailable_when_runs_dir_not_writable() {
    let root = temp("exit3-runs-dir");
    // runs 被同名文件占住，创建目录失败。
    fs::write(root.join("runs"), "blocked").unwrap();
    let input = DispatchInput {
        project: root.to_string_lossy().into(),
        text: "实现任务".into(),
        ..DispatchInput::default()
    };
    match start_dispatch(&root, input) {
        Err(StartError::Unavailable(message)) => assert!(message.contains("创建 run 目录失败")),
        other => panic!("应报引擎不可用：{other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn start_dispatch_reports_unavailable_when_spawn_fails() {
    let root = temp("exit3-spawn");
    fs::write(
        root.join("resources.json"),
        r#"{"version":1,"agents":[{"id":"codex","models":[{"id":"exec","agent":"codex"}]}]}"#,
    )
    .unwrap();
    let input = DispatchInput {
        project: root.to_string_lossy().into(),
        text: "实现任务".into(),
        targets: vec![Target {
            agent: "codex".into(),
            model: "exec".into(),
            role: "执行".into(),
            thinking: "中".into(),
            visibility: "共享".into(),
            write: "只读".into(),
            ..Target::default()
        }],
        ..DispatchInput::default()
    };
    let result = start_dispatch_with(&root, input, |_, _| Err("启动后台引擎失败：模拟".into()));
    match result {
        Err(StartError::Unavailable(message)) => assert!(message.contains("启动后台引擎失败")),
        other => panic!("应报引擎不可用：{other:?}"),
    }
    // spawn 失败时 run 落 failed，不留假 running。
    let dir = runs_dir(&root);
    let mut seen = 0;
    for entry in fs::read_dir(&dir).unwrap().flatten() {
        let run: EngineRun = serde_json::from_slice(&fs::read(entry.path()).unwrap()).unwrap();
        assert_eq!(run.status, "failed");
        assert!(run.reason.contains("启动后台引擎失败"));
        seen += 1;
    }
    assert_eq!(seen, 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cancelled_before_first_dispatch_leaves_draft_untouched() {
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), now());
    graph.nodes.insert("n1".into(), turn("n1", &["root"], "执行", &now()));
    let settings = EngineSettings::default();
    let mut sink = StubSink {
        updates: vec![],
        results: vec![],
    };
    let mut ids = || Uuid::new_v4().to_string();
    let mut clock = Utc::now;
    let outcome = run_pipeline(
        "run-c",
        "D:/p",
        &settings,
        graph,
        &["n1".to_string()],
        &[],
        "原始需求",
        &mut sink,
        &mut ids,
        &mut clock,
        &|| true,
    )
    .expect("取消不是错误");
    assert_eq!(outcome.error, CANCELLED_TEXT);
    // 没派出去过的 draft 节点不补失败桩。
    assert_eq!(outcome.graph.nodes["n1"].reply.as_ref().unwrap().status, "draft");
    assert_eq!(sink.updates.last().unwrap().status, "failed");
}

#[test]
fn file_sink_first_commit_keeps_results_written_after_load() {
    // A 的后台进程读图时 b1（另一个 run 的节点）还是 draft；之后那个 run 把
    // b1 写成 done。A 的首笔提交只能覆盖它自己改过的节点，不能把 b1 盖回 draft。
    let root = temp("merge-baseline");
    let mut base = new_graph("D:/p", "g1".into(), "root".into(), now());
    base.nodes.insert("b1".into(), turn("b1", &["root"], "执行", &now()));
    orchestration::save_graph_in(&root, &base).unwrap();

    let mut by_b = base.clone();
    by_b.nodes.get_mut("b1").unwrap().reply = Some(Reply {
        status: "done".into(),
        text: "b 结果".into(),
        ..Reply::default()
    });
    orchestration::save_graph_in(&root, &by_b).unwrap();

    let mut run_a = EngineRun { run_id: "a".into(), graph_id: "g1".into(), ..EngineRun::default() };
    let mut sink_a = FileSink::new(&root, &mut run_a, Arc::new(Mutex::new(Vec::new())), base.clone());
    let mut ga = base.clone();
    ga.nodes.insert("a1".into(), turn("a1", &["root"], "执行", &now()));
    let merged = sink_a.save_graph(&ga).unwrap().expect("FileSink 返回合并后的图");

    let disk = orchestration::load_graph_in(&root, "g1").unwrap().unwrap();
    assert_eq!(disk.nodes["b1"].reply.as_ref().unwrap().status, "done", "别的 run 写入的结果不能被旧值盖回");
    assert!(disk.nodes.contains_key("a1"));
    assert_eq!(merged.nodes["b1"].reply.as_ref().unwrap().status, "done");
    let _ = fs::remove_dir_all(root);
}

struct MergingSink {
    saves: usize,
    dispatched: Vec<String>,
}

impl PipelineSink for MergingSink {
    fn save_graph(&mut self, graph: &Graph) -> Result<Option<Graph>, String> {
        self.saves += 1;
        // 模拟合并写：第 2 次落图（循环里第一次提交）时，磁盘上另一个 run 新建了
        // 待派节点 x1。之后的落图原样返回：推进循环若没在那一刻改用合并后的
        // 图，x1 就会丢。
        let mut merged = graph.clone();
        if self.saves == 2 {
            merged
                .nodes
                .insert("x1".to_string(), turn("x1", &["root"], "执行", "2026-10-01T00:00:00.000Z"));
        }
        Ok(Some(merged))
    }
    fn update(&mut self, _update: &RunUpdate) -> Result<(), String> {
        Ok(())
    }
    fn dispatch(&mut self, request: &DispatchStageRequest) -> Result<DispatchStageResult, String> {
        let results = request
            .nodes
            .iter()
            .map(|n| {
                self.dispatched.push(n.node_id.clone());
                DispatchNodeResult {
                    node_id: n.node_id.clone(),
                    stage: n.stage,
                    status: "ok".into(),
                    text: "完成".into(),
                    ..DispatchNodeResult::default()
                }
            })
            .collect();
        Ok(DispatchStageResult {
            run_id: request.run_id.clone(),
            results,
            state_file: String::new(),
        })
    }
}

#[test]
fn pipeline_continues_from_merged_graph_and_never_dispatches_other_runs_nodes() {
    let mut graph = new_graph("D:/p", "g".into(), "root".into(), now());
    graph.nodes.insert("n1".into(), turn("n1", &["root"], "执行", &now()));
    let mut sink = MergingSink { saves: 0, dispatched: vec![] };
    let mut ids = || Uuid::new_v4().to_string();
    let mut clock = Utc::now;
    let outcome = run_pipeline(
        "run-m",
        "D:/p",
        &EngineSettings::default(),
        graph,
        &["n1".to_string()],
        &[],
        "原始需求",
        &mut sink,
        &mut ids,
        &mut clock,
        &|| false,
    )
    .expect("推进成功");
    assert_eq!(sink.dispatched, vec!["n1".to_string()], "只派本 run 的节点");
    assert!(outcome.graph.nodes.contains_key("x1"), "推进循环应改用合并后的图继续");
    assert_eq!(outcome.graph.nodes["x1"].reply.as_ref().unwrap().status, "draft");
}

#[cfg(windows)]
#[test]
fn exited_process_is_not_alive_even_while_a_handle_is_open() {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // `child` 持有句柄直到函数末尾：进程已退出，但仍能被 OpenProcess 打开。
    let mut child = std::process::Command::new("cmd")
        .args(["/C", "exit", "0"])
        .spawn()
        .expect("起子进程");
    let pid = child.id();
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.expect("打开子进程");
    let started = process_creation_time(handle).expect("读创建时间").to_string();
    unsafe {
        let _ = CloseHandle(handle);
    }
    child.wait().expect("等子进程退出");
    assert!(!process_alive(pid, &started), "已退出的进程不能判为存活");
    assert!(process_alive(std::process::id(), &own_process_started_at()), "本进程应判为存活");
    drop(child);
}

// ── fix-37-r1 R1-4：run 文件记本次预设 id ──────────────────────

#[test]
fn start_dispatch_records_preset_id_in_run_file() {
    fn spawn_ok(_: &Path, _: &[String]) -> Result<u32, String> {
        Ok(123)
    }
    fn target() -> Target {
        Target {
            agent: "codex".into(),
            model: "exec".into(),
            role: "执行".into(),
            thinking: "中".into(),
            visibility: "共享".into(),
            write: "只读".into(),
            ..Target::default()
        }
    }

    // 指定用户预设 → 记该 id。
    let root = temp("preset-id-user");
    fs::write(
        root.join("resources.json"),
        r#"{"version":1,"agents":[{"id":"codex","models":[{"id":"exec","agent":"codex"}]}]}"#,
    )
    .unwrap();
    fs::write(
        root.join("presets.json"),
        r#"{"version":1,"presets":[{"id":"u1","name":"预发回归","builtin":false,"stages":[{"nodes":[{"role":"执行","agent":"codex","model":"exec","thinking":"中","visibility":"共享","write":"只读"}]}]}]}"#,
    )
    .unwrap();
    let input = DispatchInput {
        project: root.to_string_lossy().into(),
        text: "实现任务".into(),
        preset: Some("u1".into()),
        ..DispatchInput::default()
    };
    let result = start_dispatch_with(&root, input, spawn_ok).expect("派发成功");
    let run: EngineRun = serde_json::from_slice(
        &fs::read(runs_dir(&root).join(format!("{}.json", result["run_id"].as_str().unwrap()))).unwrap(),
    )
    .unwrap();
    assert_eq!(run.preset_id, "u1");
    let _ = fs::remove_dir_all(root);

    // targets 直给 → 空串。
    let root = temp("preset-id-targets");
    fs::write(
        root.join("resources.json"),
        r#"{"version":1,"agents":[{"id":"codex","models":[{"id":"exec","agent":"codex"}]}]}"#,
    )
    .unwrap();
    let input = DispatchInput {
        project: root.to_string_lossy().into(),
        text: "实现任务".into(),
        targets: vec![target()],
        ..DispatchInput::default()
    };
    let result = start_dispatch_with(&root, input, spawn_ok).expect("派发成功");
    let run: EngineRun = serde_json::from_slice(
        &fs::read(runs_dir(&root).join(format!("{}.json", result["run_id"].as_str().unwrap()))).unwrap(),
    )
    .unwrap();
    assert_eq!(run.preset_id, "");
    let _ = fs::remove_dir_all(root);

    // 预设缺省 → 按默认经济模式记；被拒的 run 同样带上（拒绝发生在选预设
    // 之前，这里用缺 resources.json 的拒绝路验证）。
    let root = temp("preset-id-default");
    let input = DispatchInput {
        project: root.to_string_lossy().into(),
        text: "实现任务".into(),
        ..DispatchInput::default()
    };
    let result = start_dispatch_with(&root, input, spawn_ok).expect("被拒也返回 JSON");
    assert_eq!(result["accepted"].as_bool(), Some(false));
    let run: EngineRun = serde_json::from_slice(
        &fs::read(runs_dir(&root).join(format!("{}.json", result["run_id"].as_str().unwrap()))).unwrap(),
    )
    .unwrap();
    assert_eq!(run.preset_id, "builtin-economy");
    let _ = fs::remove_dir_all(root);
}
