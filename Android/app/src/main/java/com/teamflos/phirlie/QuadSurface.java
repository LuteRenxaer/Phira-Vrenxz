package com.teamflos.phirlie;

import android.content.Context;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;

import quad_native.QuadNative;

/**
 * 承载 miniquad 渲染画面的 SurfaceView。
 *
 * <p>把 Surface 生命周期、触摸、按键事件转发给原生侧。
 * 触摸 action 约定与 miniquad 一致：0=MOVE，1=UP，2=DOWN，3=CANCEL。</p>
 */
public class QuadSurface extends SurfaceView
        implements SurfaceHolder.Callback, View.OnTouchListener, View.OnKeyListener {

    public QuadSurface(Context context) {
        super(context);
        getHolder().addCallback(this);
        setFocusable(true);
        setFocusableInTouchMode(true);
        requestFocus();
        setOnTouchListener(this);
        setOnKeyListener(this);
    }

    @Override
    public void surfaceCreated(SurfaceHolder holder) {
        Surface surface = holder.getSurface();
        QuadNative.surfaceOnSurfaceCreated(surface);
    }

    @Override
    public void surfaceDestroyed(SurfaceHolder holder) {
        QuadNative.surfaceOnSurfaceDestroyed();
    }

    @Override
    public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        Surface surface = holder.getSurface();
        QuadNative.surfaceOnSurfaceChanged(surface, width, height);
    }

    @Override
    public boolean onTouch(View v, MotionEvent event) {
        int pointerCount = event.getPointerCount();
        int action = event.getActionMasked();
        long time = event.getEventTime();

        switch (action) {
            case MotionEvent.ACTION_MOVE: {
                for (int i = 0; i < pointerCount; i++) {
                    int id = event.getPointerId(i);
                    float x = event.getX(i);
                    float y = event.getY(i);
                    QuadNative.surfaceOnTouch(id, 0, x, y, time);
                }
                break;
            }
            case MotionEvent.ACTION_UP: {
                int id = event.getPointerId(0);
                QuadNative.surfaceOnTouch(id, 1, event.getX(0), event.getY(0), time);
                break;
            }
            case MotionEvent.ACTION_DOWN: {
                int id = event.getPointerId(0);
                QuadNative.surfaceOnTouch(id, 2, event.getX(0), event.getY(0), time);
                break;
            }
            case MotionEvent.ACTION_POINTER_UP: {
                int pointerIndex = event.getActionIndex();
                int id = event.getPointerId(pointerIndex);
                QuadNative.surfaceOnTouch(id, 1, event.getX(pointerIndex), event.getY(pointerIndex), time);
                break;
            }
            case MotionEvent.ACTION_POINTER_DOWN: {
                int pointerIndex = event.getActionIndex();
                int id = event.getPointerId(pointerIndex);
                QuadNative.surfaceOnTouch(id, 2, event.getX(pointerIndex), event.getY(pointerIndex), time);
                break;
            }
            case MotionEvent.ACTION_CANCEL: {
                for (int i = 0; i < pointerCount; i++) {
                    int id = event.getPointerId(i);
                    QuadNative.surfaceOnTouch(id, 3, event.getX(i), event.getY(i), time);
                }
                break;
            }
            default:
                break;
        }
        return true;
    }

    @Override
    public boolean onKey(View v, int keyCode, KeyEvent event) {
        // 音量键不消费，交回系统处理（调整系统/媒体音量），否则会被游戏吞掉无法使用。
        if (keyCode == KeyEvent.KEYCODE_VOLUME_UP || keyCode == KeyEvent.KEYCODE_VOLUME_DOWN) {
            return false;
        }
        // 长按会产生重复的 ACTION_DOWN（repeatCount>0），忽略以避免被游戏识别成连点
        if (event.getAction() == KeyEvent.ACTION_DOWN && event.getRepeatCount() > 0) {
            return true;
        }
        if (event.getAction() == KeyEvent.ACTION_DOWN && keyCode != 0) {
            QuadNative.surfaceOnKeyDown(keyCode);
        }
        if (event.getAction() == KeyEvent.ACTION_UP && keyCode != 0) {
            QuadNative.surfaceOnKeyUp(keyCode);
        }
        if (event.getAction() == KeyEvent.ACTION_UP || event.getAction() == KeyEvent.ACTION_MULTIPLE) {
            int character = event.getUnicodeChar();
            if (character == 0) {
                String chars = event.getCharacters();
                if (chars != null && !chars.isEmpty()) {
                    character = chars.charAt(0);
                }
            }
            if (character != 0) {
                QuadNative.surfaceOnCharacter(character);
            }
        }
        return true;
    }

    @Override
    public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
        outAttrs.inputType = android.text.InputType.TYPE_CLASS_TEXT;
        outAttrs.imeOptions = EditorInfo.IME_ACTION_DONE | EditorInfo.IME_FLAG_NO_FULLSCREEN;
        return new BaseInputConnection(this, true) {
            // 空的 Editable，避免 BaseInputConnection 维护组合文本状态
            private final android.text.SpannableStringBuilder fakeEditable = new android.text.SpannableStringBuilder();

            @Override
            public android.text.Editable getEditable() {
                return fakeEditable;
            }

            @Override
            public CharSequence getTextBeforeCursor(int n, int flags) {
                // 返回非空字符串，让输入法认为光标前有字符，可以删除
                return " ";
            }

            @Override
            public CharSequence getTextAfterCursor(int n, int flags) {
                return "";
            }

            @Override
            public int getCursorCapsMode(int reqModes) {
                return 0;
            }

            @Override
            public boolean commitText(CharSequence text, int newCursorPosition) {
                // 提交前先清除 BaseInputConnection 内部的组合状态，避免中文输入后退格残留拼音
                super.finishComposingText();
                // 输入法提交文本（包括中文组合输入确认），直接转发给原生侧
                if (text != null) {
                    for (int i = 0; i < text.length(); i++) {
                        char c = text.charAt(i);
                        // 处理代理对（surrogate pairs），支持 emoji 等
                        if (Character.isHighSurrogate(c) && i + 1 < text.length()) {
                            char low = text.charAt(i + 1);
                            if (Character.isLowSurrogate(low)) {
                                int codePoint = Character.toCodePoint(c, low);
                                QuadNative.surfaceOnCharacter(codePoint);
                                i++;
                                continue;
                            }
                        }
                        QuadNative.surfaceOnCharacter(c);
                    }
                }
                return true;
            }

            @Override
            public boolean deleteSurroundingText(int beforeLength, int afterLength) {
                // 输入法删除文本（退格）：以引擎 Backspace 按键事件转发，
                // 让游戏侧输入框做真实的逐字删除（0x43 = KEYCODE_DEL → Backspace）
                int count = Math.max(beforeLength, 1);
                for (int i = 0; i < count; i++) {
                    sendEngineBackspace();
                }
                return true;
            }

            @Override
            public boolean deleteSurroundingTextInCodePoints(int beforeLength, int afterLength) {
                // API 24+ 输入法常用的删除方法
                int count = Math.max(beforeLength, 1);
                for (int i = 0; i < count; i++) {
                    sendEngineBackspace();
                }
                return true;
            }

            @Override
            public boolean sendKeyEvent(KeyEvent event) {
                // 输入法发送的按键事件（包括删除、回车等），转发给原生侧
                if (event.getAction() == KeyEvent.ACTION_DOWN) {
                    QuadNative.surfaceOnKeyDown(event.getKeyCode());
                    // 同时处理可打印字符
                    int character = event.getUnicodeChar();
                    if (character != 0) {
                        QuadNative.surfaceOnCharacter(character);
                    }
                } else if (event.getAction() == KeyEvent.ACTION_UP) {
                    QuadNative.surfaceOnKeyUp(event.getKeyCode());
                }
                return true;
            }

            @Override
            public boolean performContextMenuAction(int id) {
                // 输入法的上下文菜单（全选/复制/剪切/粘贴）操作的是 IME 侧的空 Editable，
                // 游戏文本在 Rust 侧自行绘制/维护，这里回退到默认实现（操作空文本，无副作用），
                // 不能调用不存在的原生方法，否则会抛 UnsatisfiedLinkError。
                return super.performContextMenuAction(id);
            }

            @Override
            public boolean setComposingText(CharSequence text, int newCursorPosition) {
                // 不维护组合文本状态，避免退格时输入法先删联想词
                return true;
            }

            @Override
            public boolean finishComposingText() {
                return true;
            }

            @Override
            public boolean setSelection(int start, int end) {
                return true;
            }
        };
    }

    /** 以引擎 Backspace 按键事件转发一次退格（KEYCODE_DEL 会被原生侧映射为 Backspace）。 */
    private void sendEngineBackspace() {
        QuadNative.surfaceOnKeyDown(KeyEvent.KEYCODE_DEL);
        QuadNative.surfaceOnKeyUp(KeyEvent.KEYCODE_DEL);
    }

    public Surface getNativeSurface() {
        return getHolder().getSurface();
    }
}
