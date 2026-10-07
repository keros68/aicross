//! 引擎单元测试（逐夹具金标准重放在 golden.rs）。

use super::*;

fn temp(name: &str) -> PathBuf {
    let p = crate::paths::test_temp_dir().join(format!("aicross-engine-{name}-{}", Uuid::new_v4()));
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

// ── TASK-38：每次派发新建一张图 ──────────────────────────────

#[test]
fn start_dispatch_creates_new_graph_each_time() {
    fn spawn_ok(_: &Path, _: &[String]) -> Result<u32, String> {
        Ok(123)
    }
    let root = temp("graph-per-dispatch");
    fs::write(
        root.join("resources.json"),
        r#"{"version":1,"agents":[{"id":"codex","models":[{"id":"exec","agent":"codex"}]}]}"#,
    )
    .unwrap();
    let input = |text: &str| DispatchInput {
        project: root.to_string_lossy().into(),
        text: text.into(),
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
    let load = |result: &Value| {
        orchestration::load_graph_in(&root, result["graph_id"].as_str().unwrap())
            .unwrap()
            .expect("图已落盘")
    };

    let first = start_dispatch_with(&root, input("第一次任务"), spawn_ok).expect("派发成功");
    assert_eq!(first["accepted"].as_bool(), Some(true));
    // 第一张图上留一条遗留，模拟第一次 run 验收后产出的未闭合项。
    let mut graph1 = load(&first);
    let source = first["node_ids"][0].as_str().unwrap().to_string();
    graph1.nodes.insert(
        "leftover-1".into(),
        GraphNode {
            id: "leftover-1".into(),
            kind: "turn".into(),
            parents: vec![source.clone()],
            leftover: Some(Leftover {
                source_node_id: source,
                items: vec!["第一次的遗留".into()],
            }),
            created_at: now(),
            ..GraphNode::default()
        },
    );
    graph1.updated_at = now();
    orchestration::save_graph_in(&root, &graph1).unwrap();
    assert_eq!(leftover_lines(&graph1), vec!["第一次的遗留".to_string()]);

    let second = start_dispatch_with(&root, input("第二次任务"), spawn_ok).expect("派发成功");
    assert_eq!(second["accepted"].as_bool(), Some(true));
    let graph2 = load(&second);
    assert_ne!(graph2.id, graph1.id, "第二次派发应新建一张图");
    for id in graph1.nodes.keys() {
        assert!(!graph2.nodes.contains_key(id), "第二张图不应含第一次的节点 {id}");
    }
    assert!(graph2.nodes.values().all(|n| !n.user.text.contains("第一次任务")));
    // 第二次 run 的摘要遗留取自它自己的图，不含第一次的。
    assert!(leftover_lines(&graph2).is_empty());
    let run2: EngineRun = serde_json::from_slice(
        &fs::read(runs_dir(&root).join(format!("{}.json", second["run_id"].as_str().unwrap()))).unwrap(),
    )
    .unwrap();
    assert_eq!(run2.graph_id, graph2.id);
    assert!(run2.leftover.is_empty());
    // 第一张图原样保留。
    assert!(load(&first).nodes.contains_key("leftover-1"));
    let _ = fs::remove_dir_all(root);
}

// ── TASK-42：派发前检查只拦 verification == failed，冒烟超时放行 ─────────

fn task42_plan_with_verification(verification: &str) -> Plan {
    let settings = EngineSettings::default();
    let models = vec![ResourceModel {
        agent: "pi".into(),
        id: "glm-5.3".into(),
        provider: "zai".into(),
        verification: verification.into(),
        ..ResourceModel::default()
    }];
    let targets = vec![Target {
        agent: "pi".into(),
        model: "glm-5.3".into(),
        role: "执行".into(),
        ..Target::default()
    }];
    let presets: Vec<Preset> = Vec::new();
    let roles = RoleSettings::default();
    let mut git_dirty_count = move || None;
    let mut seq = 0usize;
    let mut ids = move || {
        seq += 1;
        format!("00000000-0000-4000-8000-{seq:012}")
    };
    let mut clock = move || Utc::now();
    let mut input = PlanInput {
        run_id: "r42",
        project: "D:/p42",
        text: "任务",
        preset: None,
        attachments: &[],
        targets: &targets,
        settings: &settings,
        models: &models,
        resources_error: None,
        presets: &presets,
        roles: &roles,
        host_modes: None,
        default_preset: "builtin-economy",
        active_runs: 0,
        existing: None,
        git_dirty_count: &mut git_dirty_count,
    };
    plan_dispatch(&mut input, &mut ids, &mut clock)
}

/// 冒烟超时（verification == timeout）不是验证失败：派发前检查放行；
/// verification == failed 照旧拒绝。把检查改回连 timeout 一起拦，这条就红。
#[test]
fn plan_dispatch_passes_smoke_timeout_and_still_rejects_failed() {
    let ready = task42_plan_with_verification(crate::resources::VERIFICATION_TIMEOUT);
    match ready {
        Plan::Ready { created, .. } => assert_eq!(created.len(), 1),
        Plan::Rejected { reason, .. } => panic!("冒烟超时不应拦派发：{reason}"),
    }
    let rejected = task42_plan_with_verification(crate::resources::VERIFICATION_FAILED);
    match rejected {
        Plan::Rejected { reason, .. } => assert_eq!(reason, "目标验证失败：pi/glm-5.3"),
        Plan::Ready { .. } => panic!("验证失败应照旧拦下"),
    }
    // 对照：未验证本来就放行，超时档与它同路。
    let unverified = task42_plan_with_verification(crate::resources::VERIFICATION_UNVERIFIED);
    match unverified {
        Plan::Ready { .. } => {}
        Plan::Rejected { reason, .. } => panic!("未验证不应拦派发：{reason}"),
    }
}

// ── TASK-48：资源库里「未发现」的模型 ────────────────────────────────

/// 派发前检查的注入式小夹具：模型清单与单阶段目标直给，返回 plan。
fn task48_plan_with(models: Vec<ResourceModel>, targets: Vec<Target>) -> Plan {
    task48_plan_with_preset(
        models,
        Preset {
            id: "p48".into(),
            name: "直给".into(),
            description: String::new(),
            builtin: false,
            stages: vec![orchestration::Stage { nodes: targets }],
        },
    )
}

/// 同上，预设直给：多阶段用（fix-48-r1 R2 的后面阶段用例）。
fn task48_plan_with_preset(models: Vec<ResourceModel>, preset: Preset) -> Plan {
    let settings = EngineSettings::default();
    let presets = vec![preset.clone()];
    let roles = RoleSettings::default();
    let mut git_dirty_count = move || None;
    let mut seq = 0usize;
    let mut ids = move || {
        seq += 1;
        format!("00000000-0000-4000-8000-{seq:012}")
    };
    let mut clock = move || Utc::now();
    let mut input = PlanInput {
        run_id: "r48",
        project: "D:/p48",
        text: "任务",
        preset: Some(preset.id.as_str()),
        attachments: &[],
        targets: &[],
        settings: &settings,
        models: &models,
        resources_error: None,
        presets: &presets,
        roles: &roles,
        host_modes: None,
        default_preset: "p48",
        active_runs: 0,
        existing: None,
        git_dirty_count: &mut git_dirty_count,
    };
    plan_dispatch(&mut input, &mut ids, &mut clock)
}

fn task48_pi_model(id: &str, meta: &str) -> ResourceModel {
    ResourceModel {
        agent: "pi".into(),
        id: id.into(),
        provider: "zai".into(),
        verification: crate::resources::VERIFICATION_VERIFIED.into(),
        meta: meta.into(),
        ..ResourceModel::default()
    }
}

fn task48_target_with_fallback(
    model: &str,
    fallback: Option<crate::orchestration::FallbackTarget>,
) -> Target {
    Target {
        agent: "pi".into(),
        provider: Some("zai".into()),
        model: model.into(),
        role: "执行".into(),
        thinking: "中".into(),
        visibility: "共享".into(),
        write: "只读".into(),
        fallback,
        ..Target::default()
    }
}

/// 目标是「未发现」（meta 带 MISSING_MARKER）时拒绝，原因给下一步；
/// 压在验证结果之上——同一条模型即使 verification 是 verified 也拒。
#[test]
fn task48_dispatch_rejects_missing_target_with_reason() {
    let models = vec![task48_pi_model("glm-4.7", crate::resources::MISSING_MARKER)];
    let targets = vec![task48_target_with_fallback("glm-4.7", None)];
    match task48_plan_with(models, targets) {
        Plan::Rejected { reason, .. } => {
            assert_eq!(reason, "pi/glm-4.7 在最近一次探测中未发现，请重新探测或换人选");
        }
        Plan::Ready { .. } => panic!("未发现的目标应拦下"),
    }
}

/// fix-48-r1 R1：同一模型 id 挂两个 provider、只有一个带「未发现」标记时，
/// 查找认目标的 provider——选正常那条照常派出，选带标记那条被拒；备选
/// 同口径。
#[test]
fn task48_dispatch_target_lookup_respects_provider() {
    let normal = task48_pi_model("glm-4.7", "");
    let marked = task48_pi_model("glm-4.7", crate::resources::MISSING_MARKER);
    // 目标指定正常 provider：照常派出。
    let mut target = task48_target_with_fallback("glm-4.7", None);
    target.provider = Some("opencode-go".into());
    match task48_plan_with(
        vec![task48_with_provider(&marked, "deepseek"), normal.clone()],
        vec![target],
    ) {
        Plan::Ready { .. } => {}
        Plan::Rejected { reason, .. } => panic!("正常 provider 不该被牵连：{reason}"),
    }
    // 目标指定带标记的 provider：拒绝，原因指向这条。
    let mut target = task48_target_with_fallback("glm-4.7", None);
    target.provider = Some("deepseek".into());
    match task48_plan_with(
        vec![task48_with_provider(&marked, "deepseek"), normal.clone()],
        vec![target],
    ) {
        Plan::Rejected { reason, .. } => {
            assert_eq!(reason, "pi/glm-4.7 在最近一次探测中未发现，请重新探测或换人选");
        }
        Plan::Ready { .. } => panic!("带标记的 provider 应拦下"),
    }
    // 备选同口径：备选带标记的 provider 丢弃并留说明，正常 provider 保留。
    let fallback_marked = crate::orchestration::FallbackTarget {
        agent: "pi".into(),
        provider: Some("deepseek".into()),
        model: "glm-4.7".into(),
    };
    let mut target = task48_target_with_fallback("glm-4.7", Some(fallback_marked));
    target.provider = Some("opencode-go".into());
    match task48_plan_with(
        vec![task48_with_provider(&marked, "deepseek"), normal.clone()],
        vec![target],
    ) {
        Plan::Ready { graph, notes, .. } => {
            let t = graph.nodes.values().find_map(|n| n.target.clone()).expect("target");
            assert!(t.fallback.is_none(), "带标记 provider 的备选应丢弃");
            assert_eq!(
                notes,
                vec!["备选 pi/glm-4.7 在最近一次探测中未发现，已从该节点移除".to_string()]
            );
        }
        Plan::Rejected { reason, .. } => panic!("备选带标记不该拦主目标：{reason}"),
    }
    let fallback_ok = crate::orchestration::FallbackTarget {
        agent: "pi".into(),
        provider: Some("opencode-go".into()),
        model: "glm-4.7".into(),
    };
    let mut target = task48_target_with_fallback("glm-4.7", Some(fallback_ok));
    target.provider = Some("opencode-go".into());
    match task48_plan_with(vec![task48_with_provider(&marked, "deepseek"), normal], vec![target]) {
        Plan::Ready { graph, notes, .. } => {
            let t = graph.nodes.values().find_map(|n| n.target.clone()).expect("target");
            assert_eq!(t.fallback.as_ref().expect("正常备选应保留").model, "glm-4.7");
            assert!(notes.is_empty());
        }
        Plan::Rejected { reason, .. } => panic!("正常备选不该丢：{reason}"),
    }
}

/// 复制一条模型并改 provider（夹具小工具）。
fn task48_with_provider(model: &ResourceModel, provider: &str) -> ResourceModel {
    let mut copy = model.clone();
    copy.provider = provider.into();
    copy
}

/// fix-48-r1 R2：第二阶段（盲审 / 经济模式的后面阶段）人选未发现，同样
/// 在派发前拒绝，不留到执行时才失败。
#[test]
fn task48_dispatch_rejects_missing_target_in_later_stage() {
    let models = vec![
        task48_pi_model("glm-4.7", ""),
        task48_pi_model("gone", crate::resources::MISSING_MARKER),
    ];
    let preset = Preset {
        id: "p2".into(),
        name: "两阶段".into(),
        description: String::new(),
        builtin: false,
        stages: vec![
            orchestration::Stage {
                nodes: vec![task48_target_with_fallback("glm-4.7", None)],
            },
            orchestration::Stage {
                nodes: vec![task48_target_with_fallback("gone", None)],
            },
        ],
    };
    match task48_plan_with_preset(models, preset) {
        Plan::Rejected { reason, .. } => {
            assert_eq!(reason, "pi/gone 在最近一次探测中未发现，请重新探测或换人选");
        }
        Plan::Ready { .. } => panic!("第二阶段目标未发现也应拦下"),
    }
}

/// fix-48-r1 R2：第二阶段备选未发现同样从该节点移除并留说明，主目标照常。
#[test]
fn task48_dispatch_drops_missing_fallback_in_later_stage() {
    let models = vec![
        task48_pi_model("glm-4.7", ""),
        task48_pi_model("gone", crate::resources::MISSING_MARKER),
    ];
    let preset = Preset {
        id: "p2".into(),
        name: "两阶段".into(),
        description: String::new(),
        builtin: false,
        stages: vec![
            orchestration::Stage {
                nodes: vec![task48_target_with_fallback("glm-4.7", None)],
            },
            orchestration::Stage {
                nodes: vec![task48_target_with_fallback(
                    "glm-4.7",
                    Some(crate::orchestration::FallbackTarget {
                        agent: "pi".into(),
                        provider: Some("zai".into()),
                        model: "gone".into(),
                    }),
                )],
            },
        ],
    };
    match task48_plan_with_preset(models, preset) {
        Plan::Ready { graph, notes, .. } => {
            assert_eq!(
                notes,
                vec!["备选 pi/gone 在最近一次探测中未发现，已从该节点移除".to_string()]
            );
            let with_fallback = graph
                .nodes
                .values()
                .filter_map(|n| n.target.as_ref())
                .filter(|t| t.fallback.is_some())
                .count();
            assert_eq!(with_fallback, 0, "第二阶段备选应已从节点移除");
        }
        Plan::Rejected { reason, .. } => panic!("第二阶段备选未发现不应拦主目标：{reason}"),
    }
}

/// 备选是「未发现」时只丢掉这个备选，主目标照常派出，notes 里留说明；
/// 备选正常时原样保留、notes 为空。
#[test]
fn task48_dispatch_drops_missing_fallback_but_keeps_normal_one() {
    let make_targets = |fallback: Option<crate::orchestration::FallbackTarget>| {
        vec![task48_target_with_fallback("glm-4.7", fallback)]
    };
    let models = vec![
        task48_pi_model("glm-4.7", ""),
        task48_pi_model("gone", crate::resources::MISSING_MARKER),
    ];
    let fallback_missing = crate::orchestration::FallbackTarget {
        agent: "pi".into(),
        provider: Some("zai".into()),
        model: "gone".into(),
    };
    match task48_plan_with(models, make_targets(Some(fallback_missing))) {
        Plan::Ready { graph, notes, .. } => {
            let target = graph.nodes.values().find_map(|n| n.target.clone()).expect("target");
            assert!(target.fallback.is_none(), "未发现的备选应丢掉");
            assert_eq!(
                notes,
                vec!["备选 pi/gone 在最近一次探测中未发现，已从该节点移除".to_string()]
            );
        }
        Plan::Rejected { reason, .. } => panic!("备选未发现不应拦主目标：{reason}"),
    }
    let fallback_ok = crate::orchestration::FallbackTarget {
        agent: "pi".into(),
        provider: Some("zai".into()),
        model: "glm-4.7".into(),
    };
    let models_ok = vec![task48_pi_model("glm-4.7", "")];
    match task48_plan_with(models_ok, make_targets(Some(fallback_ok))) {
        Plan::Ready { graph, notes, .. } => {
            let target = graph.nodes.values().find_map(|n| n.target.clone()).expect("target");
            assert_eq!(target.fallback.as_ref().expect("fallback kept").model, "glm-4.7");
            assert!(notes.is_empty());
        }
        Plan::Rejected { reason, .. } => panic!("正常备选不应丢：{reason}"),
    }
}

/// 端到端：备选未发现时 run 文件带 notes，落图的备选已丢掉，响应也带 notes。
#[test]
fn task48_start_dispatch_writes_fallback_note_to_run_file() {
    fn spawn_ok(_: &Path, _: &[String]) -> Result<u32, String> {
        Ok(123)
    }
    let root = temp("task48-fallback-note");
    fs::write(
        root.join("resources.json"),
        r#"{"version":1,"agents":[{"id":"pi","models":[
            {"id":"glm-4.7","agent":"pi","provider":"zai","status":"已接入","verification":"verified","resolved_id":"glm-4.7","resolved_source":"identity"},
            {"id":"gone","agent":"pi","provider":"zai","status":"已接入","verification":"verified","resolved_id":"gone","resolved_source":"identity","meta":"未在事实源中"}
        ]}]}
"#,
    )
    .unwrap();
    let input = DispatchInput {
        project: root.to_string_lossy().into(),
        text: "实现任务".into(),
        targets: vec![task48_target_with_fallback(
            "glm-4.7",
            Some(crate::orchestration::FallbackTarget {
                agent: "pi".into(),
                provider: Some("zai".into()),
                model: "gone".into(),
            }),
        )],
        ..DispatchInput::default()
    };
    let result = start_dispatch_with(&root, input, spawn_ok).expect("派发成功");
    assert_eq!(result["accepted"].as_bool(), Some(true));
    assert_eq!(result["notes"].as_array().map(Vec::len), Some(1));
    let run: EngineRun = serde_json::from_slice(
        &fs::read(runs_dir(&root).join(format!("{}.json", result["run_id"].as_str().unwrap()))).unwrap(),
    )
    .unwrap();
    assert_eq!(
        run.notes,
        vec!["备选 pi/gone 在最近一次探测中未发现，已从该节点移除".to_string()]
    );
    let graph = orchestration::load_graph_in(&root, &run.graph_id)
        .unwrap()
        .expect("graph");
    let target = graph.nodes.values().find_map(|n| n.target.clone()).expect("target");
    assert!(target.fallback.is_none(), "落图的备选应已丢掉");
    let _ = fs::remove_dir_all(root);
}

// ── TASK-46：模式人选与默认模式 ─────────────────────────────────────

fn task46_seat(agent: &str, provider: Option<&str>, model: &str) -> crate::orchestration::SeatChoice {
    crate::orchestration::SeatChoice {
        agent: agent.into(),
        provider: provider.map(str::to_string),
        model: model.into(),
        ..crate::orchestration::SeatChoice::default()
    }
}

/// 四种来源各一条：预设写死 / 位置人选 / 角色默认 / 未指定。
#[test]
fn task46_fill_seats_reports_all_four_sources() {
    let mut preset = crate::orchestration::builtin_presets()
        .into_iter()
        .find(|preset| preset.id == "builtin-economy")
        .unwrap();
    // 出方案写死人选；执行留给角色默认；验收留给位置人选；验证什么都不留。
    preset.stages[0].nodes[0].agent = "codex".into();
    preset.stages[0].nodes[0].model = "hardwired".into();
    let roles = RoleSettings {
        execution: crate::orchestration::RoleProfile {
            agent: "codex".into(),
            provider: "".into(),
            model: "exec".into(),
            tier: "低".into(),
            thinking: "低".into(),
        },
        ..RoleSettings::default()
    };
    let seats = vec![
        vec![None],
        vec![None],
        vec![Some(task46_seat("pi", Some("zai"), "glm-4.7")), None],
    ];
    let (filled, sources) = fill_seats(&preset, &roles, Some(&seats));
    assert_eq!(sources, vec![vec!["预设"], vec!["角色"], vec!["位置", "未指定"]]);
    let planning = &filled.stages[0].nodes[0];
    assert_eq!((planning.agent.as_str(), planning.model.as_str()), ("codex", "hardwired"));
    let execution = &filled.stages[1].nodes[0];
    assert_eq!((execution.agent.as_str(), execution.model.as_str()), ("codex", "exec"));
    assert_eq!(execution.tier.as_deref(), Some("低"));
    let acceptance = &filled.stages[2].nodes[0];
    assert_eq!((acceptance.agent.as_str(), acceptance.model.as_str()), ("pi", "glm-4.7"));
    assert!(filled.stages[2].nodes[1].model.is_empty());
}

/// tier / thinking 空串沿用预设值，非空覆盖；fallback 写进 Target.fallback。
#[test]
fn task46_fill_seats_overrides_tier_thinking_and_writes_fallback() {
    let preset = crate::orchestration::builtin_presets()
        .into_iter()
        .find(|preset| preset.id == "builtin-blind")
        .unwrap();
    let seats = vec![vec![
        Some(crate::orchestration::SeatChoice {
            tier: "高".into(),
            fallback: Some(crate::orchestration::FallbackTarget {
                agent: "claude".into(),
                provider: None,
                model: "sonnet".into(),
            }),
            ..task46_seat("pi", Some("zai"), "glm-4.7")
        }),
        Some(task46_seat("codex", None, "exec")),
        None,
    ]];
    let (filled, _) = fill_seats(&preset, &RoleSettings::default(), Some(&seats));
    let first = &filled.stages[0].nodes[0];
    assert_eq!(first.tier.as_deref(), Some("高"), "seat tier 非空应覆盖预设");
    assert_eq!(first.thinking, "中", "seat thinking 空串应沿用预设");
    let fallback = first.fallback.as_ref().unwrap();
    assert_eq!((fallback.agent.as_str(), fallback.model.as_str()), ("claude", "sonnet"));
    assert!(fallback.provider.is_none());
    let second = &filled.stages[0].nodes[1];
    assert_eq!(second.thinking, "中", "seat 未给 thinking 应沿用预设");
    assert!(second.fallback.is_none());
}

/// seats 与预设长度对不上：阶段多出的忽略、位置缺的按 null。
#[test]
fn task46_fill_seats_tolerates_shape_mismatch() {
    let preset = crate::orchestration::builtin_presets()
        .into_iter()
        .find(|preset| preset.id == "builtin-blind")
        .unwrap();
    // 阶段多出一个（多出的忽略），首个阶段位置只有 1 个（缺的按 null）。
    let seats = vec![
        vec![Some(task46_seat("pi", Some("zai"), "glm-4.7"))],
        vec![Some(task46_seat("codex", None, "exec"))],
    ];
    let (filled, sources) = fill_seats(&preset, &RoleSettings::default(), Some(&seats));
    assert_eq!(sources, vec![vec!["位置", "未指定", "未指定"]]);
    assert_eq!(filled.stages.len(), 1);
    assert_eq!(filled.stages[0].nodes.len(), 3);
}

/// fix-46-r1 R5：agent 或 model 为空的人选按未设处理，继续走角色 / 未指定；
/// thinking 非法值忽略，不覆盖预设值。
#[test]
fn task46_fill_seats_ignores_empty_seat_and_invalid_thinking() {
    let preset = crate::orchestration::builtin_presets()
        .into_iter()
        .find(|preset| preset.id == "builtin-blind")
        .unwrap();
    let roles = RoleSettings {
        acceptance: crate::orchestration::RoleProfile {
            agent: "codex".into(),
            provider: "".into(),
            model: "accept".into(),
            tier: "".into(),
            thinking: "高".into(),
        },
        ..RoleSettings::default()
    };
    let seats = vec![vec![
        // agent 为空：按未设处理；盲审的角色档整份跳过，落在「未指定」。
        Some(task46_seat("", Some("zai"), "glm-4.7")),
        // model 为空：同样按未设处理。
        Some(task46_seat("pi", Some("zai"), " ")),
        // thinking 非法：位置生效，但 thinking 沿用预设的「中」。
        Some(crate::orchestration::SeatChoice {
            thinking: "超高".into(),
            ..task46_seat("pi", Some("zai"), "glm-4.7")
        }),
    ]];
    let (filled, sources) = fill_seats(&preset, &roles, Some(&seats));
    assert_eq!(sources, vec![vec!["未指定", "未指定", "位置"]]);
    assert!(filled.stages[0].nodes[0].model.is_empty());
    assert!(filled.stages[0].nodes[1].model.is_empty());
    assert_eq!(filled.stages[0].nodes[2].model, "glm-4.7");
    assert_eq!(filled.stages[0].nodes[2].thinking, "中", "非法 thinking 应沿用预设");
    // 对照：经济模式里 agent 为空的人选落到「角色」档（验收角色已设）。
    let economy = crate::orchestration::builtin_presets()
        .into_iter()
        .find(|preset| preset.id == "builtin-economy")
        .unwrap();
    let seats = vec![
        vec![None],
        vec![None],
        vec![Some(task46_seat(" ", None, "x")), None],
    ];
    let (filled, sources) = fill_seats(&economy, &roles, Some(&seats));
    assert_eq!(sources[2], vec!["角色", "角色"], "验收与验证都走验收角色");
    assert_eq!(filled.stages[2].nodes[0].model, "accept");
    assert_eq!(filled.stages[2].nodes[1].model, "accept");
}

/// fix-46-r1 R9：盲审模式在 roles.json 设了角色时，没有位置人选的来源仍是
/// 「未指定」（规则 3 对盲审整份跳过，不因设了角色而变）。
#[test]
fn task46_blind_with_roles_but_no_seats_stays_unspecified() {
    let preset = crate::orchestration::builtin_presets()
        .into_iter()
        .find(|preset| preset.id == "builtin-blind")
        .unwrap();
    let roles = RoleSettings {
        planning: crate::orchestration::RoleProfile {
            agent: "codex".into(),
            provider: "".into(),
            model: "plan".into(),
            tier: "".into(),
            thinking: "".into(),
        },
        execution: crate::orchestration::RoleProfile {
            agent: "codex".into(),
            provider: "".into(),
            model: "exec".into(),
            tier: "".into(),
            thinking: "".into(),
        },
        acceptance: crate::orchestration::RoleProfile {
            agent: "codex".into(),
            provider: "".into(),
            model: "accept".into(),
            tier: "".into(),
            thinking: "".into(),
        },
        ..RoleSettings::default()
    };
    let (filled, sources) = fill_seats(&preset, &roles, None);
    assert_eq!(sources, vec![vec!["未指定", "未指定", "未指定"]]);
    for node in &filled.stages[0].nodes {
        assert!(node.model.is_empty());
    }
}

/// fix-46-r1 R9：同一阶段的人选位置数多于预设节点数，多出的忽略。
#[test]
fn task46_fill_seats_ignores_extra_positions_in_row() {
    let preset = crate::orchestration::builtin_presets()
        .into_iter()
        .find(|preset| preset.id == "builtin-blind")
        .unwrap();
    let seats = vec![vec![
        None,
        Some(task46_seat("pi", Some("zai"), "glm-4.7")),
        None,
        Some(task46_seat("codex", None, "extra")),
        Some(task46_seat("claude", None, "extra-2")),
    ]];
    let (filled, sources) = fill_seats(&preset, &RoleSettings::default(), Some(&seats));
    assert_eq!(sources, vec![vec!["未指定", "位置", "未指定"]]);
    assert_eq!(filled.stages[0].nodes.len(), 3);
    assert!(filled.stages[0].nodes[2].model.is_empty());
}

/// 用户预设写死的人选不被 modes.json 覆盖（规则 1 优先于规则 2）。
#[test]
fn task46_fill_seats_keeps_user_preset_hardwired_target() {
    let preset = crate::orchestration::Preset {
        id: "u1".into(),
        name: "自定义".into(),
        description: String::new(),
        builtin: false,
        stages: vec![crate::orchestration::Stage {
            nodes: vec![Target {
                agent: "codex".into(),
                model: "hardwired".into(),
                role: "执行".into(),
                thinking: "中".into(),
                visibility: "共享".into(),
                write: "只读".into(),
                ..Target::default()
            }],
        }],
    };
    let seats = vec![vec![Some(task46_seat("pi", Some("zai"), "glm-4.7"))]];
    let (filled, sources) = fill_seats(&preset, &RoleSettings::default(), Some(&seats));
    assert_eq!(sources, vec![vec!["预设"]]);
    let target = &filled.stages[0].nodes[0];
    assert_eq!((target.agent.as_str(), target.model.as_str()), ("codex", "hardwired"));
}

/// 盲审模式有人选时能派出（run 记 builtin-blind）；没有人选时仍被拒。
#[test]
fn task46_blind_dispatch_with_seats_is_accepted_and_without_is_rejected() {
    fn spawn_ok(_: &Path, _: &[String]) -> Result<u32, String> {
        Ok(123)
    }
    let root = temp("task46-blind");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("resources.json"),
        r#"{"version":1,"agents":[{"id":"pi","models":[{"id":"glm-4.7","agent":"pi","provider":"zai","verification":"verified"}]}]}"#,
    )
    .unwrap();
    let input = |root: &Path| DispatchInput {
        project: root.to_string_lossy().into(),
        text: "盲审这个改动".into(),
        preset: Some("builtin-blind".into()),
        ..DispatchInput::default()
    };
    let run_preset_id = |root: &Path, result: &Value| -> String {
        let run: EngineRun = serde_json::from_slice(
            &fs::read(runs_dir(root).join(format!("{}.json", result["run_id"].as_str().unwrap()))).unwrap(),
        )
        .unwrap();
        run.preset_id
    };
    // 没有人选：仍被派发前检查拒绝。
    let result = start_dispatch_with(&root, input(&root), spawn_ok).unwrap();
    assert_eq!(result["accepted"].as_bool(), Some(false));
    assert!(result["reason"].as_str().unwrap().contains("没有可派发目标"), "{result}");
    assert_eq!(run_preset_id(&root, &result), "builtin-blind");

    // 有人选：三个位置都按人选派出（modes.json v2：配置挂在主 agent 下）。
    let modes = modes_for_host(
        "claude",
        crate::orchestration::HostModes {
            seats: indexmap::IndexMap::from([(
                "builtin-blind".into(),
                vec![vec![
                    Some(task46_seat("pi", Some("zai"), "glm-4.7")),
                    Some(task46_seat("pi", Some("zai"), "glm-4.7")),
                    Some(task46_seat("pi", Some("zai"), "glm-4.7")),
                ]],
            )]),
            ..crate::orchestration::HostModes::default()
        },
    );
    crate::orchestration::save_modes_in(&root, &modes).unwrap();
    let result = start_dispatch_with(&root, input(&root), spawn_ok).unwrap();
    assert_eq!(result["accepted"].as_bool(), Some(true), "{result}");
    assert_eq!(run_preset_id(&root, &result), "builtin-blind");
    let graph = orchestration::load_graph_in(&root, result["graph_id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    let ids: Vec<String> = result["node_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids.len(), 3);
    for id in &ids {
        let target = graph.nodes.get(id).unwrap().target.as_ref().unwrap();
        assert_eq!((target.agent.as_str(), target.model.as_str()), ("pi", "glm-4.7"));
    }
    let _ = fs::remove_dir_all(root);
}

/// 默认模式：未设 → 经济；设了盲审 → 盲审；指向不存在的预设 → 经济；
/// 显式 --preset 优先于默认模式。run 文件的 preset_id 记实际用的那个。
#[test]
fn task46_default_mode_resolved_for_dispatch_without_preset() {
    fn spawn_ok(_: &Path, _: &[String]) -> Result<u32, String> {
        Ok(123)
    }
    /// 每个用例一个新根目录：stub 拉起的 run 停在启动窗口内，同根连发
    /// 会触在飞上限。
    fn dispatch_case(default_mode: Option<&str>, preset: Option<&str>) -> (PathBuf, Value) {
        let root = temp("task46-default");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"agents":[
                {"id":"pi","models":[{"id":"glm-4.7","agent":"pi","provider":"zai","verification":"verified"}]},
                {"id":"codex","models":[{"id":"exec","agent":"codex"}]}
            ]}"#,
        )
        .unwrap();
        // 经济模式三个角色都有人，未设角色默认时会被「没有可派发目标」拒绝。
        fs::write(
            root.join("roles.json"),
            r#"{"version":1,"planning":{"agent":"codex","model":"exec","thinking":"低"},"execution":{"agent":"codex","model":"exec","thinking":"低"},"acceptance":{"agent":"pi","provider":"zai","model":"glm-4.7","thinking":"中"}}"#,
        )
        .unwrap();
        if let Some(default_mode) = default_mode {
            // 盲审预设没有角色默认可补，设成默认模式时必须带人选才能派出。
            let modes = modes_for_host(
                "claude",
                crate::orchestration::HostModes {
                    default_mode: default_mode.into(),
                    seats: indexmap::IndexMap::from([(
                        "builtin-blind".into(),
                        vec![vec![
                            Some(task46_seat("pi", Some("zai"), "glm-4.7")),
                            Some(task46_seat("pi", Some("zai"), "glm-4.7")),
                            Some(task46_seat("pi", Some("zai"), "glm-4.7")),
                        ]],
                    )]),
                    ..crate::orchestration::HostModes::default()
                },
            );
            crate::orchestration::save_modes_in(&root, &modes).unwrap();
        }
        let input = DispatchInput {
            project: root.to_string_lossy().into(),
            text: "任务".into(),
            preset: preset.map(str::to_string),
            ..DispatchInput::default()
        };
        let result = start_dispatch_with(&root, input, spawn_ok).unwrap();
        (root, result)
    }
    let run_preset_id = |root: &Path, result: &Value| -> String {
        let run: EngineRun = serde_json::from_slice(
            &fs::read(runs_dir(root).join(format!("{}.json", result["run_id"].as_str().unwrap()))).unwrap(),
        )
        .unwrap();
        run.preset_id
    };

    // 未设默认模式 → 经济。
    let (root, result) = dispatch_case(None, None);
    assert_eq!(result["accepted"].as_bool(), Some(true), "{result}");
    assert_eq!(run_preset_id(&root, &result), "builtin-economy");
    let _ = fs::remove_dir_all(root);

    // 设了盲审 → 盲审。
    let (root, result) = dispatch_case(Some("builtin-blind"), None);
    assert_eq!(result["accepted"].as_bool(), Some(true), "{result}");
    assert_eq!(run_preset_id(&root, &result), "builtin-blind");
    let _ = fs::remove_dir_all(root);

    // 指向不存在的预设 → 经济。
    let (root, result) = dispatch_case(Some("builtin-ghost"), None);
    assert_eq!(result["accepted"].as_bool(), Some(true), "{result}");
    assert_eq!(run_preset_id(&root, &result), "builtin-economy");
    let _ = fs::remove_dir_all(root);

    // 显式 --preset 优先于默认模式。（引擎层不解析别名，别名在命令行入口
    // build_request 里解析。）
    let (root, result) = dispatch_case(Some("builtin-blind"), Some("builtin-economy"));
    assert_eq!(result["accepted"].as_bool(), Some(true), "{result}");
    assert_eq!(run_preset_id(&root, &result), "builtin-economy");
    let _ = fs::remove_dir_all(root);
}

/// modes.json v2：把一个主 agent 的配置包成整份文件（测试夹具用）。
fn modes_for_host(host: &str, section: crate::orchestration::HostModes) -> ModesFile {
    ModesFile {
        version: crate::orchestration::MODES_VERSION,
        hosts: indexmap::IndexMap::from([(host.to_string(), section)]),
    }
}

/// default_preset_id：空与未知回经济，存在的预设（含用户预设）生效，trim 后匹配。
#[test]
fn task46_default_preset_id_rules() {
    let presets = crate::orchestration::builtin_presets();
    let modes = |default_mode: &str| crate::orchestration::HostModes {
        default_mode: default_mode.into(),
        ..crate::orchestration::HostModes::default()
    };
    assert_eq!(default_preset_id(None, &presets), "builtin-economy");
    assert_eq!(default_preset_id(Some(&modes("")), &presets), "builtin-economy");
    assert_eq!(default_preset_id(Some(&modes("builtin-ghost")), &presets), "builtin-economy");
    assert_eq!(default_preset_id(Some(&modes("builtin-blind")), &presets), "builtin-blind");
    assert_eq!(default_preset_id(Some(&modes(" builtin-blind ")), &presets), "builtin-blind");
}

/// 复审追加：人选有效而备选的 agent 或 model 为空时，备选按未设处理。
#[test]
fn task46_fill_seats_ignores_empty_fallback() {
    let preset = crate::orchestration::builtin_presets()
        .into_iter()
        .find(|preset| preset.id == "builtin-blind")
        .unwrap();
    let fallback = |agent: &str, model: &str| crate::orchestration::FallbackTarget {
        agent: agent.into(),
        provider: None,
        model: model.into(),
    };
    let seats = vec![vec![
        Some(crate::orchestration::SeatChoice {
            fallback: Some(fallback("", "sonnet")),
            ..task46_seat("pi", Some("zai"), "glm-4.7")
        }),
        Some(crate::orchestration::SeatChoice {
            fallback: Some(fallback("claude", " ")),
            ..task46_seat("pi", Some("zai"), "glm-4.7")
        }),
        Some(crate::orchestration::SeatChoice {
            fallback: Some(fallback("claude", "sonnet")),
            ..task46_seat("pi", Some("zai"), "glm-4.7")
        }),
    ]];
    let (filled, sources) = fill_seats(&preset, &RoleSettings::default(), Some(&seats));
    assert_eq!(sources, vec![vec!["位置", "位置", "位置"]]);
    assert!(filled.stages[0].nodes[0].fallback.is_none());
    assert!(filled.stages[0].nodes[1].fallback.is_none());
    assert_eq!(filled.stages[0].nodes[2].fallback.as_ref().unwrap().model, "sonnet");
}

// ── TASK-56：位置数量可调 ─────────────────────────────────────────────

/// 盖一个单材料节点的图，返回新建节点的文本（按创建序）；focus 是评审
/// 模式的生效视角名单（TASK-59），其它模式传空。
fn task56_stamp_texts(preset: &Preset, focus: &[String]) -> Vec<String> {
    let mut graph = Graph {
        version: 1,
        id: "g56".into(),
        root: "root".into(),
        ..Graph::default()
    };
    graph.nodes.insert(
        "root".into(),
        GraphNode {
            id: "root".into(),
            kind: "material".into(),
            user: Message {
                text: "材料".into(),
                ..Message::default()
            },
            ..GraphNode::default()
        },
    );
    let mut seq = 0usize;
    let mut ids = move || {
        seq += 1;
        format!("n{seq}")
    };
    let base = Utc::now();
    let (next, created) = stamp_with(&graph, "root", preset, 3, focus, &mut ids, base);
    created
        .iter()
        .map(|id| next.nodes.get(id).unwrap().user.text.clone())
        .collect()
}

fn task56_shaped(id: &str, count: usize) -> Preset {
    let preset = crate::orchestration::builtin_presets()
        .into_iter()
        .find(|preset| preset.id == id)
        .unwrap();
    crate::orchestration::shape_builtin_preset(&preset, count)
}

/// 位置文本随调整后的形状生成：评审视角按「安全 → 性能 → 需求覆盖」
/// 依次取；盲审与经济的验证位置都用验证路由文本；经济减到 0 只剩验收。
#[test]
fn task56_stamp_texts_follow_adjusted_shape() {
    // 评审：默认 3 个位置，三段视角文本依次出现；减到 2 只剩前两段。
    let texts = task56_stamp_texts(&task56_shaped("builtin-jury", 3), &[]);
    assert_eq!(texts.len(), 3);
    assert!(texts[0].starts_with("安全评审："), "{}", texts[0]);
    assert!(texts[1].starts_with("性能评审："), "{}", texts[1]);
    assert!(texts[2].starts_with("需求覆盖评审："), "{}", texts[2]);
    let texts = task56_stamp_texts(&task56_shaped("builtin-jury", 2), &[]);
    assert_eq!(texts.len(), 2);
    assert!(texts[0].starts_with("安全评审："), "{}", texts[0]);
    assert!(texts[1].starts_with("性能评审："), "{}", texts[1]);
    // 盲审：验收不覆盖，验证位置都套验证路由文本。
    let route = "盲审：独立审查同一份产物，只看材料与验证命令输出，不看验收结论。";
    let texts = task56_stamp_texts(&task56_shaped("builtin-blind", 1), &[]);
    assert_eq!(texts, vec!["".to_string(), route.to_string()]);
    let texts = task56_stamp_texts(&task56_shaped("builtin-blind", 4), &[]);
    assert_eq!(
        texts,
        vec![
            "".to_string(),
            route.to_string(),
            route.to_string(),
            route.to_string(),
            route.to_string(),
        ]
    );
    // 经济：验证减到 0 只剩验收；加到 2 两路都是验证路由文本。
    let texts = task56_stamp_texts(&task56_shaped("builtin-economy", 0), &[]);
    assert_eq!(texts, vec!["".to_string(), "".to_string(), "".to_string()]);
    let texts = task56_stamp_texts(&task56_shaped("builtin-economy", 2), &[]);
    assert_eq!(
        texts,
        vec![
            "".to_string(),
            "".to_string(),
            "".to_string(),
            route.to_string(),
            route.to_string(),
        ]
    );
}

// ── TASK-59：评审视角可选 ──────────────────────────────────────

/// 视角文本：内置视角查表（规范 / 正确性 / 可维护性也在表里），自定义
/// 文字套统一模板，空串给空。
#[test]
fn task59_jury_focus_text_lookup_and_template() {
    assert_eq!(
        jury_focus_text("安全"),
        "安全评审：只从安全视角审查这批材料，指出风险、暴露面与缓释建议。"
    );
    assert_eq!(
        jury_focus_text("规范"),
        "规范评审：只从项目规范视角审查这批材料：对照仓库已有的编码规范与约定（如 AGENTS.md、CONTRIBUTING.md、lint 配置），指出违反之处与修改建议。"
    );
    assert_eq!(
        jury_focus_text("正确性"),
        "正确性评审：只从正确性视角审查这批材料，指出逻辑错误、遗漏的边界条件与反例。"
    );
    assert_eq!(
        jury_focus_text("可维护性"),
        "可维护性评审：只从可维护性视角审查这批材料，指出重复、命名、耦合与可以简化之处。"
    );
    assert_eq!(
        jury_focus_text("可靠性"),
        "可靠性评审：只从可靠性视角审查这批材料，指出问题与改进建议。"
    );
    assert_eq!(jury_focus_text(""), "");
}

/// fix-59-r1：视角文字校验——换行、制表在 trim 前就拒绝（首尾也算）；
/// Unicode 格式字符（零宽、双向覆盖与隔离、BOM）拒绝；首尾普通空白去掉；
/// 空白串规整为空串（表示用默认）。
#[test]
fn task59_normalize_focus_text_rules() {
    assert_eq!(normalize_focus_text("  可靠性 ").unwrap(), "可靠性");
    assert_eq!(normalize_focus_text("   ").unwrap(), "");
    assert_eq!(normalize_focus_text("").unwrap(), "");
    for bad in ["规范\n", "\n规范", "\r规范", "规范\t", "两\n行"] {
        let message = normalize_focus_text(bad).unwrap_err();
        assert!(message.contains("控制字符"), "{bad:?} → {message}");
    }
    assert!(normalize_focus_text("\u{1b}[31m红").unwrap_err().contains("控制字符"));
    for bad in [
        "规\u{200b}范",
        "规\u{200f}范",
        "\u{202a}规范",
        "\u{202e}规范",
        "\u{2066}规范\u{2069}",
        "\u{feff}规范",
        "规\u{ad}范",
    ] {
        let message = normalize_focus_text(bad).unwrap_err();
        assert!(message.contains("格式字符"), "{bad:?} → {message}");
        assert!(message.contains("\\u{"), "{bad:?} → {message}");
    }
    assert!(normalize_focus_text(&"长".repeat(20)).is_ok());
    assert!(normalize_focus_text(&"长".repeat(21)).unwrap_err().contains("1–20 字"));
}

/// fix-59-r1：读出口用同一套校验——手写 modes.json 里带换行、超长、
/// 格式字符的视角回落默认表，不报错；合格的照常生效（首尾空白去掉）。
#[test]
fn task59_focus_from_handwritten_modes_falls_back() {
    let root = crate::paths::test_temp_dir()
        .join(format!("aicross-t59-focus-read-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let long = "长".repeat(21);
    let text = format!(
        r#"{{"version":2,"hosts":{{"claude":{{"focus":{{"builtin-jury":["两\n行"," 可靠性 ","{long}"]}}}},"codex":{{"focus":{{"builtin-jury":["规\u200b范","\u202e性能","正确性\t"]}}}}}}}}"#
    );
    std::fs::write(root.join("modes.json"), text).unwrap();
    let modes = crate::orchestration::modes_from(&root);
    let preset = task56_shaped("builtin-jury", 3);
    let section = crate::orchestration::host_section(&modes, "claude");
    assert!(section.is_some(), "手写的 modes.json 要能读到");
    let names = jury_focus_names(section, &preset);
    assert_eq!(
        names,
        vec!["安全".to_string(), "可靠性".to_string(), "需求覆盖".to_string()]
    );
    // 盖章文本同一份名单：不合格的位置是默认文本，合格的按自定义模板。
    let texts = task56_stamp_texts(&preset, &names);
    assert!(texts[0].starts_with("安全评审："), "{}", texts[0]);
    assert!(texts[1].starts_with("可靠性评审："), "{}", texts[1]);
    assert!(texts[2].starts_with("需求覆盖评审："), "{}", texts[2]);
    // 零宽、双向覆盖、制表：全部回默认表。
    let section = crate::orchestration::host_section(&modes, "codex");
    assert!(section.is_some());
    assert_eq!(
        jury_focus_names(section, &preset),
        jury_focus_names(None, &preset)
    );
    let _ = std::fs::remove_dir_all(root);
}

/// 生效视角名单：配置里设了的生效（含内置名与自定义文字），空串或缺的
/// 下标回默认表；数量调小后多出的项不进名单，调大后恢复。
#[test]
fn task59_jury_focus_names_follow_config_and_shape() {
    let section = |focus: Vec<String>| crate::orchestration::HostModes {
        focus: indexmap::IndexMap::from([("builtin-jury".to_string(), focus)]),
        ..crate::orchestration::HostModes::default()
    };
    let preset3 = task56_shaped("builtin-jury", 3);
    // 改成「规范」＋自定义文字，空串回默认。
    let names = jury_focus_names(
        Some(&section(vec![
            "规范".into(),
            String::new(),
            "可靠性".into(),
        ])),
        &preset3,
    );
    assert_eq!(names, vec!["规范".to_string(), "性能".to_string(), "可靠性".to_string()]);
    // 全部空串与没有配置一致，都回默认表。
    assert_eq!(
        jury_focus_names(Some(&section(vec![String::new(), String::new(), String::new()])), &preset3),
        jury_focus_names(None, &preset3)
    );
    assert_eq!(
        jury_focus_names(None, &preset3),
        vec!["安全".to_string(), "性能".to_string(), "需求覆盖".to_string()]
    );
    // 数量调到 2：多出的第三项不进名单，前两位不变；调回 3 后恢复。
    let preset2 = task56_shaped("builtin-jury", 2);
    let focus = vec!["规范".to_string(), String::new(), "可靠性".to_string()];
    assert_eq!(
        jury_focus_names(Some(&section(focus.clone())), &preset2),
        vec!["规范".to_string(), "性能".to_string()]
    );
    assert_eq!(
        jury_focus_names(Some(&section(focus)), &preset3)[2],
        "可靠性"
    );
    // 非评审模式给空名单。
    assert!(jury_focus_names(None, &task56_shaped("builtin-blind", 2)).is_empty());
}

/// 盖章用上配置的视角：改成「规范」的节点是规范文本，自定义文字套模板，
/// 空串回默认。
#[test]
fn task59_stamp_texts_follow_focus_overrides() {
    let section = crate::orchestration::HostModes {
        focus: indexmap::IndexMap::from([(
            "builtin-jury".to_string(),
            vec!["规范".to_string(), String::new(), "可靠性".to_string()],
        )]),
        ..crate::orchestration::HostModes::default()
    };
    let preset = task56_shaped("builtin-jury", 3);
    let texts = task56_stamp_texts(&preset, &jury_focus_names(Some(&section), &preset));
    assert!(texts[0].starts_with("规范评审：只从项目规范视角"), "{}", texts[0]);
    assert!(texts[1].starts_with("性能评审："), "{}", texts[1]);
    assert!(
        texts[2].starts_with("可靠性评审：只从可靠性视角审查这批材料"),
        "{}",
        texts[2]
    );
}

/// 派发按调整后的形状建节点：盲审 counts=1 时只建验收 + 1 路验证，
/// 验证节点的正文仍是验证路由文本。
#[test]
fn task56_dispatch_builds_nodes_per_counts() {
    fn spawn_ok(_: &Path, _: &[String]) -> Result<u32, String> {
        Ok(123)
    }
    let root = temp("task56-dispatch");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("resources.json"),
        r#"{"version":1,"agents":[{"id":"pi","models":[{"id":"glm-4.7","agent":"pi","provider":"zai","verification":"verified"}]}]}"#,
    )
    .unwrap();
    let seat = task46_seat("pi", Some("zai"), "glm-4.7");
    let modes = modes_for_host(
        "claude",
        crate::orchestration::HostModes {
            counts: indexmap::IndexMap::from([("builtin-blind".to_string(), 1)]),
            seats: indexmap::IndexMap::from([(
                "builtin-blind".into(),
                vec![vec![Some(seat.clone()), Some(seat)]],
            )]),
            ..crate::orchestration::HostModes::default()
        },
    );
    crate::orchestration::save_modes_in(&root, &modes).unwrap();
    let input = DispatchInput {
        project: root.to_string_lossy().into(),
        text: "盲审这个改动".into(),
        preset: Some("builtin-blind".into()),
        ..DispatchInput::default()
    };
    let result = start_dispatch_with(&root, input, spawn_ok).unwrap();
    assert_eq!(result["accepted"].as_bool(), Some(true), "{result}");
    let graph = orchestration::load_graph_in(&root, result["graph_id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    let ids: Vec<String> = result["node_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids.len(), 2, "验收加 1 路验证：{ids:?}");
    let roles: Vec<&str> = ids
        .iter()
        .map(|id| graph.nodes.get(id).unwrap().target.as_ref().unwrap().role.as_str())
        .collect();
    assert_eq!(roles, ["验收", "验证"]);
    let route = "盲审：独立审查同一份产物，只看材料与验证命令输出，不看验收结论。";
    assert_eq!(graph.nodes.get(&ids[1]).unwrap().user.text, route);
    let _ = fs::remove_dir_all(root);
}

/// 派发（start_dispatch_with → plan_dispatch）用上该主 agent 配置的评审
/// 视角：改成「规范」后节点正文是规范文本，自定义文字套模板，清空后回
/// 默认；数量调到 2 后多出的视角项不影响。counts 与 focus 同一条路径，
/// 都从 modes.json 的 host 段取。
#[test]
fn task59_dispatch_stamps_configured_focus() {
    fn spawn_ok(_: &Path, _: &[String]) -> Result<u32, String> {
        Ok(123)
    }
    let root = temp("task59-dispatch-focus");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("resources.json"),
        r#"{"version":1,"agents":[{"id":"pi","models":[{"id":"glm-4.7","agent":"pi","provider":"zai","verification":"verified"}]}]}"#,
    )
    .unwrap();
    let seat = task46_seat("pi", Some("zai"), "glm-4.7");
    let write_modes = |count: usize, focus: &[&str]| {
        let list: Vec<String> = focus.iter().map(|name| name.to_string()).collect();
        let modes = modes_for_host(
            "claude",
            crate::orchestration::HostModes {
                counts: indexmap::IndexMap::from([("builtin-jury".to_string(), count as u32)]),
                focus: indexmap::IndexMap::from([("builtin-jury".to_string(), list)]),
                seats: indexmap::IndexMap::from([(
                    "builtin-jury".into(),
                    vec![vec![Some(seat.clone()); count]],
                )]),
                ..crate::orchestration::HostModes::default()
            },
        );
        crate::orchestration::save_modes_in(&root, &modes).unwrap();
    };
    let texts_of = |focus: &[&str], count: usize| {
        write_modes(count, focus);
        let input = DispatchInput {
            project: root.to_string_lossy().into(),
            text: "评审这个改动".into(),
            preset: Some("builtin-jury".into()),
            ..DispatchInput::default()
        };
        let result = start_dispatch_with(&root, input, spawn_ok).unwrap();
        assert_eq!(result["accepted"].as_bool(), Some(true), "{result}");
        let graph = orchestration::load_graph_in(&root, result["graph_id"].as_str().unwrap())
            .unwrap()
            .unwrap();
        result["node_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| graph.nodes.get(id.as_str().unwrap()).unwrap().user.text.clone())
            .collect::<Vec<String>>()
    };
    // 内置名、自定义文字、空串回默认各占一位。
    let texts = texts_of(&["规范", "", "可靠性"], 3);
    assert!(texts[0].starts_with("规范评审：只从项目规范视角"), "{}", texts[0]);
    assert!(texts[1].starts_with("性能评审："), "{}", texts[1]);
    assert!(
        texts[2].starts_with("可靠性评审：只从可靠性视角审查这批材料"),
        "{}",
        texts[2]
    );
    // 清空后回默认表。
    let texts = texts_of(&["", "", ""], 3);
    assert!(texts[0].starts_with("安全评审："), "{}", texts[0]);
    assert!(texts[1].starts_with("性能评审："), "{}", texts[1]);
    assert!(texts[2].starts_with("需求覆盖评审："), "{}", texts[2]);
    // 数量调到 2：多出的第三项不影响前两位。
    let texts = texts_of(&["规范", "", "多余的"], 2);
    assert_eq!(texts.len(), 2);
    assert!(texts[0].starts_with("规范评审："), "{}", texts[0]);
    assert!(texts[1].starts_with("性能评审："), "{}", texts[1]);
    let _ = fs::remove_dir_all(root);
}

/// 自定义模式真正走一次派发（start_dispatch_with → plan_dispatch）：引擎
/// 按 id 取到 presets.json 里的用户预设，按它的阶段建节点，节点 target
/// 的角色与人选与预设、modes.json 一致。
#[test]
fn task56_custom_preset_dispatches_with_own_shape() {
    fn spawn_ok(_: &Path, _: &[String]) -> Result<u32, String> {
        Ok(123)
    }
    let node = |role: &str, visibility: &str, write: &str| crate::orchestration::Target {
        role: role.into(),
        visibility: visibility.into(),
        write: write.into(),
        thinking: "中".into(),
        ..crate::orchestration::Target::default()
    };
    let custom = crate::orchestration::Preset {
        id: "custom-flow".into(),
        name: "流水模式".into(),
        description: "自定义的三个阶段".into(),
        builtin: false,
        stages: vec![
            crate::orchestration::Stage {
                nodes: vec![node("执行", "共享", "单写者")],
            },
            crate::orchestration::Stage {
                nodes: vec![node("验收", "盲", "只读"), node("验证", "盲", "只读")],
            },
        ],
    };
    let root = temp("task56-custom-preset");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("resources.json"),
        r#"{"version":1,"agents":[
            {"id":"pi","models":[{"id":"glm-4.7","agent":"pi","provider":"zai","verification":"verified"}]},
            {"id":"codex","models":[{"id":"gpt-5.2","agent":"codex","verification":"verified"}]}
        ]}"#,
    )
    .unwrap();
    crate::orchestration::save_presets_in(
        &root,
        &crate::orchestration::PresetFile {
            version: 1,
            presets: vec![custom],
        },
    )
    .unwrap();
    let seat = task46_seat("pi", Some("zai"), "glm-4.7");
    let accept = task46_seat("codex", None, "gpt-5.2");
    let modes = modes_for_host(
        "claude",
        crate::orchestration::HostModes {
            seats: indexmap::IndexMap::from([(
                "custom-flow".into(),
                vec![vec![Some(seat.clone())], vec![Some(accept), Some(seat)]],
            )]),
            ..crate::orchestration::HostModes::default()
        },
    );
    crate::orchestration::save_modes_in(&root, &modes).unwrap();
    let input = DispatchInput {
        project: root.to_string_lossy().into(),
        text: "按自定义模式派发".into(),
        preset: Some("custom-flow".into()),
        ..DispatchInput::default()
    };
    let result = start_dispatch_with(&root, input, spawn_ok).unwrap();
    assert_eq!(result["accepted"].as_bool(), Some(true), "{result}");
    let graph = orchestration::load_graph_in(&root, result["graph_id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    let ids: Vec<String> = result["node_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids.len(), 3, "阶段形状 1 + 2：{ids:?}");
    let summary: Vec<(String, String, String)> = ids
        .iter()
        .map(|id| {
            let target = graph.nodes.get(id).unwrap().target.as_ref().unwrap();
            (
                target.role.clone(),
                target.agent.clone(),
                target.model.clone(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("执行".to_string(), "pi".to_string(), "glm-4.7".to_string()),
            ("验收".to_string(), "codex".to_string(), "gpt-5.2".to_string()),
            ("验证".to_string(), "pi".to_string(), "glm-4.7".to_string()),
        ],
        "{summary:?}"
    );
    let _ = fs::remove_dir_all(root);
}

/// fix-57-r1 8：盖章后派发构造出的每个节点提示词都以如实汇报要求收尾
/// （所有角色同一处追加），用户原文在提示词里原样保留，不被改动。
#[test]
fn task57_r1_dispatch_prompts_end_with_honest_reporting() {
    let preset = crate::orchestration::builtin_presets()
        .into_iter()
        .find(|preset| preset.id == "builtin-economy")
        .unwrap();
    let mut graph = Graph {
        version: 1,
        id: "g57".into(),
        root: "root".into(),
        ..Graph::default()
    };
    graph.nodes.insert(
        "root".into(),
        GraphNode {
            id: "root".into(),
            kind: "material".into(),
            user: Message {
                text: "实现导出功能".into(),
                ..Message::default()
            },
            ..GraphNode::default()
        },
    );
    let mut seq = 0usize;
    let mut ids = move || {
        seq += 1;
        format!("n{seq}")
    };
    let (graph, created) = stamp_with(&graph, "root", &preset, 3, &[], &mut ids, Utc::now());
    let request = build_request(
        "run-1",
        "D:/p",
        &EngineSettings::default(),
        &graph,
        &created,
        &[],
        "实现导出功能",
    );
    assert_eq!(request.nodes.len(), created.len());
    for node in &request.nodes {
        assert!(
            node.prompt.ends_with(HONEST_REPORTING),
            "提示词应以如实汇报要求收尾：{}",
            node.prompt
        );
        // 用户原文不被改动：原样出现在提示词里。
        assert!(node.prompt.contains("[用户]\n实现导出功能"), "{}", node.prompt);
        // 常量只定义一处：追加的就是那个常量，不含其它变体。
        assert_eq!(node.prompt.matches(HONEST_REPORTING).count(), 1, "{}", node.prompt);
    }
}
