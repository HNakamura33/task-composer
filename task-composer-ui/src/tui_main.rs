//! Task Composer TUI - ターミナルベースのDAG管理
//!
//! ratauiを使用したTUI実装（グラフ表示付き）

use std::io;
use std::fs;
use std::collections::HashMap;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap, canvas::{Canvas, Line as CanvasLine, Rectangle}},
    Frame, Terminal,
};
use task_composer_core::types::{Task, Status};
use task_composer_core::dag::DAG;

fn main() -> Result<(), io::Error> {
    // CLI引数からファイルパスを取得
    let args: Vec<String> = std::env::args().collect();
    let file_path = args.get(1).map(|s| s.as_str());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    // ファイルが指定されていれば読み込み、なければサンプルを表示
    if let Some(path) = file_path {
        if let Err(e) = app.load_file(path) {
            // エラー時はサンプルを読み込んでエラーメッセージを表示
            app.load_sample();
            app.error_message = Some(format!("Failed to load {}: {}", path, e));
        }
    } else {
        app.load_sample();
    }

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err:?}");
    }

    Ok(())
}

struct App {
    tasks: Vec<Task>,
    edges: HashMap<String, Vec<String>>,
    selected_index: usize,
    list_state: ListState,
    view_mode: ViewMode,
    node_positions: HashMap<String, (f64, f64)>,
    error_message: Option<String>,
    file_path: Option<String>,
}

#[derive(PartialEq, Clone, Copy)]
enum ViewMode {
    List,
    Graph,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            tasks: Vec::new(),
            edges: HashMap::new(),
            selected_index: 0,
            list_state,
            view_mode: ViewMode::List,
            node_positions: HashMap::new(),
            error_message: None,
            file_path: None,
        }
    }

    /// ファイルからDAGを読み込む
    fn load_file(&mut self, path: &str) -> Result<(), String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let dag = DAG::from_json(&contents)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        // DAGのnodesからタスクを取得
        self.tasks = dag.nodes.values().cloned().collect();
        // task_idでソート
        self.tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id));

        // DAGからエッジ情報を構築（親→子の関係）
        self.edges = dag.edges;

        self.file_path = Some(path.to_string());
        self.calculate_positions();

        if !self.tasks.is_empty() {
            self.selected_index = 0;
            self.list_state.select(Some(0));
        }

        Ok(())
    }

    fn load_sample(&mut self) {
        self.tasks = vec![
            Task {
                task_id: "1".to_string(),
                name: "Initial Setup".to_string(),
                description: "Setup project environment".to_string(),
                priority: 1,
                status: Status::Completed,
                prompt: "Initialize the project".to_string(),
                executor: "log".to_string(),
                dependencies: vec![],
                role: Default::default(),
                args: serde_json::Value::Null,
                inputs: serde_json::Value::Null,
            },
            Task {
                task_id: "2".to_string(),
                name: "Build Frontend".to_string(),
                description: "Compile frontend assets".to_string(),
                priority: 2,
                status: Status::InProgress,
                prompt: "Build frontend".to_string(),
                executor: "log".to_string(),
                dependencies: vec!["1".to_string()],
                role: Default::default(),
                args: serde_json::Value::Null,
                inputs: serde_json::Value::Null,
            },
            Task {
                task_id: "3".to_string(),
                name: "Build Backend".to_string(),
                description: "Compile backend services".to_string(),
                priority: 2,
                status: Status::Pending,
                prompt: "Build backend".to_string(),
                executor: "log".to_string(),
                dependencies: vec!["1".to_string()],
                role: Default::default(),
                args: serde_json::Value::Null,
                inputs: serde_json::Value::Null,
            },
            Task {
                task_id: "4".to_string(),
                name: "Deploy".to_string(),
                description: "Deploy to production".to_string(),
                priority: 3,
                status: Status::Pending,
                prompt: "Deploy application".to_string(),
                executor: "log".to_string(),
                dependencies: vec!["2".to_string(), "3".to_string()],
                role: Default::default(),
                args: serde_json::Value::Null,
                inputs: serde_json::Value::Null,
            },
        ];

        self.edges.insert("1".to_string(), vec!["2".to_string(), "3".to_string()]);
        self.edges.insert("2".to_string(), vec!["4".to_string()]);
        self.edges.insert("3".to_string(), vec!["4".to_string()]);

        self.calculate_positions();
    }

    fn calculate_positions(&mut self) {
        // レイヤーベースのレイアウト計算
        let mut layers: HashMap<String, usize> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        for task in &self.tasks {
            in_degree.insert(task.task_id.clone(), 0);
        }
        for to_ids in self.edges.values() {
            for to_id in to_ids {
                *in_degree.entry(to_id.clone()).or_insert(0) += 1;
            }
        }

        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        for id in &queue {
            layers.insert(id.clone(), 0);
        }

        let mut i = 0;
        while i < queue.len() {
            let current = queue[i].clone();
            let current_layer = *layers.get(&current).unwrap_or(&0);
            i += 1;

            if let Some(children) = self.edges.get(&current) {
                for child in children {
                    let child_layer = layers.entry(child.clone()).or_insert(0);
                    *child_layer = (*child_layer).max(current_layer + 1);

                    if let Some(deg) = in_degree.get_mut(child) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 && !queue.contains(child) {
                            queue.push(child.clone());
                        }
                    }
                }
            }
        }

        // レイヤーごとにノードを配置
        let mut layer_nodes: HashMap<usize, Vec<String>> = HashMap::new();
        for (task_id, layer) in &layers {
            layer_nodes.entry(*layer).or_default().push(task_id.clone());
        }

        let _max_layer = layer_nodes.keys().max().copied().unwrap_or(0);

        for (layer, nodes) in &layer_nodes {
            let y = 80.0 - (*layer as f64) * 25.0; // 上から下へ
            let total_nodes = nodes.len();
            for (idx, task_id) in nodes.iter().enumerate() {
                let x = if total_nodes == 1 {
                    50.0
                } else {
                    20.0 + (idx as f64) * (60.0 / (total_nodes - 1) as f64)
                };
                self.node_positions.insert(task_id.clone(), (x, y));
            }
        }
    }

    fn next(&mut self) {
        if self.tasks.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.tasks.len();
        self.list_state.select(Some(self.selected_index));
    }

    fn previous(&mut self) {
        if self.tasks.is_empty() {
            return;
        }
        self.selected_index = if self.selected_index == 0 {
            self.tasks.len() - 1
        } else {
            self.selected_index - 1
        };
        self.list_state.select(Some(self.selected_index));
    }

    fn selected_task(&self) -> Option<&Task> {
        self.tasks.get(self.selected_index)
    }

    fn toggle_view(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::List => ViewMode::Graph,
            ViewMode::Graph => ViewMode::List,
        };
    }
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Tab | KeyCode::Char('g') => app.toggle_view(),
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    match app.view_mode {
        ViewMode::List => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(main_chunks[0]);

            render_task_list(f, app, chunks[0]);
            render_detail_panel(f, app, chunks[1]);
        }
        ViewMode::Graph => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(main_chunks[0]);

            render_graph(f, app, chunks[0]);
            render_task_list(f, app, chunks[1]);
        }
    }

    render_help(f, main_chunks[1], app);
}

fn render_task_list(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .tasks
        .iter()
        .map(|task| {
            let status_icon = match task.status {
                Status::Pending => "○",
                Status::InProgress => "◐",
                Status::Completed => "●",
            };
            let status_color = match task.status {
                Status::Pending => Color::Gray,
                Status::InProgress => Color::Blue,
                Status::Completed => Color::Green,
            };

            let line = Line::from(vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
                Span::styled(
                    format!("#{} ", task.task_id),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(&task.name),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Tasks ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(99, 102, 241))),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(30, 41, 59))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_detail_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(99, 102, 241)));

    if let Some(task) = app.selected_task() {
        let status_text = match task.status {
            Status::Pending => ("Pending", Color::Gray),
            Status::InProgress => ("In Progress", Color::Blue),
            Status::Completed => ("Completed", Color::Green),
        };

        let text = vec![
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
                Span::raw(&task.task_id),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&task.name, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(status_text.0, Style::default().fg(status_text.1)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Executor: ", Style::default().fg(Color::DarkGray)),
                Span::raw(&task.executor),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Dependencies: ", Style::default().fg(Color::DarkGray)),
                Span::raw(if task.dependencies.is_empty() {
                    "None".to_string()
                } else {
                    task.dependencies.join(", ")
                }),
            ]),
            Line::from(""),
            Line::from(Span::styled("Description:", Style::default().fg(Color::DarkGray))),
            Line::from(Span::raw(&task.description)),
        ];

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
        f.render_widget(paragraph, area);
    } else {
        let paragraph = Paragraph::new("No task selected")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(paragraph, area);
    }
}

fn render_graph(f: &mut Frame, app: &App, area: Rect) {
    let selected_id = app.selected_task().map(|t| t.task_id.clone());

    let canvas = Canvas::default()
        .block(
            Block::default()
                .title(" DAG Graph ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(99, 102, 241))),
        )
        .x_bounds([0.0, 100.0])
        .y_bounds([0.0, 100.0])
        .paint(|ctx| {
            // エッジを描画
            for (from_id, to_ids) in &app.edges {
                if let Some(&(from_x, from_y)) = app.node_positions.get(from_id) {
                    for to_id in to_ids {
                        if let Some(&(to_x, to_y)) = app.node_positions.get(to_id) {
                            ctx.draw(&CanvasLine {
                                x1: from_x,
                                y1: from_y - 3.0,
                                x2: to_x,
                                y2: to_y + 3.0,
                                color: Color::Rgb(71, 85, 105),
                            });
                        }
                    }
                }
            }

            // ノードを描画
            for task in &app.tasks {
                if let Some(&(x, y)) = app.node_positions.get(&task.task_id) {
                    let is_selected = selected_id.as_ref() == Some(&task.task_id);

                    let node_color = match task.status {
                        Status::Pending => Color::Gray,
                        Status::InProgress => Color::Blue,
                        Status::Completed => Color::Green,
                    };

                    let border_color = if is_selected {
                        Color::Rgb(129, 140, 248)
                    } else {
                        Color::Rgb(51, 65, 85)
                    };

                    // ノードの背景（矩形）
                    ctx.draw(&Rectangle {
                        x: x - 8.0,
                        y: y - 3.0,
                        width: 16.0,
                        height: 6.0,
                        color: border_color,
                    });

                    // ステータスインジケーター（小さな矩形）
                    ctx.draw(&Rectangle {
                        x: x - 8.0,
                        y: y - 3.0,
                        width: 1.5,
                        height: 6.0,
                        color: node_color,
                    });

                    // ノード名をラベルとして表示
                    let label = if task.name.len() > 10 {
                        format!("{}...", &task.name[..7])
                    } else {
                        task.name.clone()
                    };
                    ctx.print(x - 6.0, y, Line::from(Span::styled(
                        format!("#{} {}", task.task_id, label),
                        Style::default().fg(if is_selected { Color::White } else { Color::Rgb(148, 163, 184) }),
                    )));
                }
            }
        });

    f.render_widget(canvas, area);
}

fn render_help(f: &mut Frame, area: Rect, app: &App) {
    let mode_text = match app.view_mode {
        ViewMode::List => "List",
        ViewMode::Graph => "Graph",
    };

    let file_info = if let Some(path) = &app.file_path {
        format!(" 📄 {} ", path)
    } else {
        " Sample ".to_string()
    };

    let help_text = Line::from(vec![
        Span::styled(" q ", Style::default().bg(Color::Rgb(30, 41, 59))),
        Span::raw(" Quit "),
        Span::styled(" ↑↓/jk ", Style::default().bg(Color::Rgb(30, 41, 59))),
        Span::raw(" Navigate "),
        Span::styled(" Tab/g ", Style::default().bg(Color::Rgb(30, 41, 59))),
        Span::raw(" Toggle View "),
        Span::styled(format!(" {} ", mode_text), Style::default().fg(Color::Rgb(129, 140, 248))),
        Span::styled(file_info, Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(help_text).style(Style::default().fg(Color::DarkGray));
    f.render_widget(paragraph, area);
}
