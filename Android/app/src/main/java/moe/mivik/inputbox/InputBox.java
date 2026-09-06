package moe.mivik.inputbox;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.Context;
import android.graphics.Color;
import android.os.Build;
import android.text.InputType;
import android.widget.EditText;
import android.widget.LinearLayout;

import com.teamflos.phirlie.MainActivity;
import com.teamflos.phirlie.R;

/**
 * inputbox（Rust 输入框后端）所需的原生对话框实现。
 *
 * <p><b>重要：</b>包名 {@code moe.mivik.inputbox}、类名 {@code InputBox} 以及
 * {@code showInput}/{@code inputCallback} 的方法名/签名必须与 Rust 侧
 * {@code inputbox::backend::Android} 保持一致，否则会抛 {@code NoSuchMethodError}。</p>
 *
 * <p>Rust 侧通过静态方法 {@link #showInput} 弹出输入框；用户确认/取消后由 Java
 * 调用原生静态方法 {@link #inputCallback} 把结果回传给 Rust。</p>
 */
public final class InputBox {

    private InputBox() {
    }

    /** 设置白色光标（兼容 API 23+）。 */
    private static void setCursorColor(EditText input) {
        try {
            if (Build.VERSION.SDK_INT >= 29) {
                input.setTextCursorDrawable(input.getContext().getDrawable(R.drawable.text_cursor));
            } else {
                java.lang.reflect.Field f = android.widget.TextView.class.getDeclaredField("mCursorDrawableRes");
                f.setAccessible(true);
                f.setInt(input, R.drawable.text_cursor);
            }
        } catch (Exception ignored) {
        }
    }

    // 由 .so 导出：Java_moe_mivik_inputbox_InputBox_inputCallback
    public static native void inputCallback(long callback, String text);

    /**
     * 弹出输入框。由 Rust 侧以静态方式调用。
     *
     * @return 成功返回 null；失败返回错误信息字符串（Rust 侧将非 null 视为错误）。
     */
    public static String showInput(
            final long callback,
            final String title,
            final String prompt,
            final String defaultText,
            final String okLabel,
            final String cancelLabel,
            final String mode,
            final boolean autoWrap,
            final boolean scrollToEnd) {

        final Activity activity = MainActivity.getInstance();
        if (activity == null) {
            return "no activity";
        }

        activity.runOnUiThread(() -> {
            try {
                final EditText input = new EditText(activity);
                input.setText(defaultText == null ? "" : defaultText);
                input.setSelection(input.getText().length());
                // 深色背景：文字/提示/光标/高亮用白色，避免黑色文字看不见
                input.setTextColor(Color.WHITE);
                input.setHintTextColor(Color.parseColor("#B3FFFFFF"));
                input.setHighlightColor(Color.WHITE);
                setCursorColor(input);

                if (mode != null && mode.contains("multiline")) {
                    input.setSingleLine(false);
                    input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
                    input.setGravity(android.view.Gravity.TOP | android.view.Gravity.START);
                } else {
                    input.setSingleLine(true);
                    input.setInputType(InputType.TYPE_CLASS_TEXT);
                }

                LinearLayout container = new LinearLayout(activity);
                int pad = (int) (16 * activity.getResources().getDisplayMetrics().density);
                container.setPadding(pad, pad, pad, pad);
                container.addView(input, new LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT));

                AlertDialog dialog = new AlertDialog.Builder(activity)
                        .setTitle(title == null || title.isEmpty() ? "输入" : title)
                        .setMessage(prompt == null || prompt.isEmpty() ? null : prompt)
                        .setView(container)
                        .setCancelable(false)
                        .setPositiveButton(okLabel == null || okLabel.isEmpty() ? "确定" : okLabel,
                                (d, which) -> inputCallback(callback, input.getText().toString()))
                        .setNegativeButton(cancelLabel == null || cancelLabel.isEmpty() ? "取消" : cancelLabel,
                                (d, which) -> inputCallback(callback, null))
                        .create();
                dialog.setCanceledOnTouchOutside(false);
                dialog.show();
            } catch (Exception e) {
                inputCallback(callback, null);
            }
        });
        return null;
    }
}
