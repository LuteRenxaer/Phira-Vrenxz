package quad_native;

import android.app.Activity;
import android.net.Uri;
import android.view.Surface;

import com.teamflos.phirlie.MainActivity;

/**
 * Phira-Vrenxz（miniquad/macroquad 引擎）的原生桥接类。
 *
 * <p><b>重要：</b>类的包名和名称必须保持为 {@code quad_native.QuadNative}，
 * 因为 Rust 侧通过非静态注册的方式导出了形如
 * {@code Java_quad_1native_QuadNative_xxx} 的 JNI 符号
 * （JNI 名称混淆规则中 {@code _1} 表示 {@code quad_native} 里的下划线），
 * JVM 会依据「完整限定类名 + 方法名」在已加载的 .so 中查找对应符号。
 * 改包名/类名将导致 {@code UnsatisfiedLinkError}。
 *
 * <p>所有方法均为 {@code native static}，与 miniquad 官方模板保持一致。
 * 请保证每个声明的 native 方法都在 .so 中有对应导出；无法配对的声明应删除，
 * 否则调用时会抛 {@code UnsatisfiedLinkError}。
 */
public final class QuadNative {

    static {
        // 加载 libphira_vrenxz.so。
        // 如需更换 .so，请把新的 lib<LIBRARY_NAME>.so 放入
        // app/src/main/jniLibs/<abi>/ 后重新打包（见 MainActivity.LIBRARY_NAME）。
        System.loadLibrary(MainActivity.LIBRARY_NAME);
    }

    private QuadNative() {
    }

    // ------------------------------------------------------------------
    // miniquad 生命周期 / 渲染
    // ------------------------------------------------------------------

    /** 初始化 JNI 上下文（ndk_context），必须在 activityOnCreate 之前调用。 */
    public static native void initializeContext(Activity activity);

    /** 释放 JNI 上下文。 */
    public static native void releaseContext();

    /** 触发 Rust 侧 quad_main()，启动渲染线程。 */
    public static native void activityOnCreate(Activity activity);

    public static native void activityOnResume();

    public static native void activityOnPause();

    public static native void activityOnDestroy();

    public static native void surfaceOnSurfaceCreated(Surface surface);

    public static native void surfaceOnSurfaceDestroyed();

    public static native void surfaceOnSurfaceChanged(Surface surface, int width, int height);

    public static native void surfaceOnTouch(int touchId, int action, float x, float y, long time);

    public static native void surfaceOnKeyDown(int keycode);

    public static native void surfaceOnKeyUp(int keycode);

    public static native void surfaceOnCharacter(int character);

    // ------------------------------------------------------------------
    // Phira-Vrenxz 扩展
    // ------------------------------------------------------------------

    /** 初始化输入框后端（inputbox）。 */
    public static native void initializeEnvironment();

    /** 应用自身的 onPause/onResume/onDestroy 钩子（防沉迷等）。 */
    public static native void prprActivityOnPause();

    public static native void prprActivityOnResume();

    public static native void prprActivityOnDestroy();

    /** 设置数据目录（对应 getFilesDir()）。 */
    public static native void setDataPath(String path);

    /** 设置缓存目录 / TMPDIR。 */
    public static native void setTempDir(String path);

    /** 设置 DPI。 */
    public static native void setDpi(int dpi);

    /** 通知原生侧「已选择文件」，path 为应用私有目录内的绝对路径。 */
    public static native void setChosenFile(String path);

    /** 标记当前选择用于导入谱面。 */
    public static native void markImport();

    /** 标记当前选择用于导入资源包。 */
    public static native void markImportRespack();

    /** 标记当前选择用于自动导入（谱面/资源包自动识别）。 */
    public static native void markAutoImport();

    /** 设置输入框结果文本。 */
    public static native void setInputText(String text);

    /** 设置深链接（phira://）的多人启动参数。 */
    public static native void setStartupArgs(String join, String create, String server);

    /** 处理导出文件描述符（SAF 导出的 Uri + fd）。 */
    public static native void processExportFd(Uri uri, int fd);
}
