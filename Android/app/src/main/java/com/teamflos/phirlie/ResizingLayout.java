package com.teamflos.phirlie;

import android.app.Activity;
import android.graphics.Color;
import android.graphics.Insets;
import android.os.Build;
import android.view.View;
import android.view.WindowInsets;
import android.widget.LinearLayout;

/**
 * 用于承载 {@link QuadSurface} 的布局，处理系统栏 / IME 的 insets，
 * 在横屏弹出键盘等场景避免内容被遮挡。
 */
public class ResizingLayout extends LinearLayout implements View.OnApplyWindowInsetsListener {

    private boolean imeResizeEnabled = true;

    public ResizingLayout(Activity activity) {
        super(activity);
        setBackgroundColor(Color.BLACK);
        setOnApplyWindowInsetsListener(this);
    }

    /** 设置是否响应 IME insets（输入框激活时禁用，避免游戏画面被压缩） */
    public void setImeResizeEnabled(boolean enabled) {
        this.imeResizeEnabled = enabled;
        if (!enabled) {
            // 立即清除 IME padding，恢复全屏
            setPadding(getPaddingLeft(), getPaddingTop(), getPaddingRight(), 0);
        } else {
            // 重新请求 insets 应用
            requestApplyInsets();
        }
    }

    @Override
    public WindowInsets onApplyWindowInsets(View v, WindowInsets insets) {
        if (Build.VERSION.SDK_INT >= 30) {
            Insets imeInsets = insets.getInsets(WindowInsets.Type.ime());
            Insets sysInsets = insets.getInsets(WindowInsets.Type.systemBars());

            int bottomPadding = sysInsets.bottom;
            if (imeResizeEnabled && imeInsets.bottom > 0) {
                bottomPadding = imeInsets.bottom;
            }
            setPadding(
                    sysInsets.left,
                    sysInsets.top,
                    sysInsets.right,
                    bottomPadding
            );
        }
        return insets;
    }
}
