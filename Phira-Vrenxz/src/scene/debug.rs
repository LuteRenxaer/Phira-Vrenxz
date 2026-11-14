use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{screen_aspect, draw_text_aligned},
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::Ui,
};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;

const COMMAND_FILE: &str = "F:/phirLie/debug_command.txt";

pub struct DebugCommandListener {
    last_check: f32,
}

impl DebugCommandListener {
    pub fn new() -> Self {
        Self {
            last_check: 0.0,
        }
    }

    pub fn update(&mut self, tm: &mut TimeManager) -> Option<String> {
        let now = tm.now() as f32;
        if now - self.last_check < 0.5 {
            return None;
        }
        self.last_check = now;

        let path = PathBuf::from(COMMAND_FILE);
        if !path.exists() {
            return None;
        }

        match fs::read_to_string(&path) {
            Ok(content) => {
                let command = content.trim().to_string();
                // 删除命令文件
                let _ = fs::remove_file(&path);
                Some(command)
            }
            Err(_) => None,
        }
    }
}

// 全局的命令监听器
thread_local! {
    pub static DEBUG_LISTENER: RefCell<DebugCommandListener> = RefCell::new(DebugCommandListener::new());
}
