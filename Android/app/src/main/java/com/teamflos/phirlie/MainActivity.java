package com.teamflos.phirlie;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.util.Log;
import android.view.View;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.view.inputmethod.InputMethodManager;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.List;
import java.util.UUID;

import quad_native.QuadNative;

/**
 * Phira-Vrenxz 的 Java 外壳主界面。
 *
 * <p>职责：</p>
 * <ul>
 *   <li>加载并启动 {@code libphira_vrenxz.so}（miniquad 渲染引擎 + 游戏本体）；</li>
 *   <li>把 Surface、触摸/按键事件、生命周期回调转发给原生侧；</li>
 *   <li>桥接原生侧需要的能力：文件选择（导入谱面/资源包）、SAF 导出、
 *       打开链接、剪贴板、输入框等。</li>
 * </ul>
 *
 * <p>最小兼容 Android 6.0（API 23，minSdk = 23）。</p>
 */
public class MainActivity extends Activity {

    /** 原生库名（不含 lib 前缀与 .so 后缀）。如需替换 .so，请同步修改这里。 */
    public static final String LIBRARY_NAME = "phira_vrenxz";

    private static final String TAG = "PhiraVrenxz";

    private static final int REQ_OPEN_FILE = 1001;
    private static final int REQ_CREATE_FILE = 1002;
    private static final int REQ_OPEN_FOLDER = 1003;

    /** 当前 Activity 实例，供原生/输入框在 UI 线程执行。 */
    private static MainActivity instance;

    /** 供 InputBox 等组件获取应用上下文。 */
    private static Context appContext;

    private QuadSurface view;
    private ResizingLayout layout;

    public static MainActivity getInstance() {
        return instance;
    }

    public static Context getAppContext() {
        return appContext;
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        instance = this;
        appContext = getApplicationContext();

        requestWindowFeature(Window.FEATURE_NO_TITLE);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);

        // 1. 初始化 JNI 上下文（ndk_context），必须先于 activityOnCreate。
        QuadNative.initializeContext(this);

        // 2. 设置数据/缓存目录与 DPI，先于渲染线程读取 data.json。
        String filesDir = getFilesDir().getAbsolutePath();
        String cacheDir = getCacheDir().getAbsolutePath();
        Log.i(TAG, "filesDir=" + filesDir);
        Log.i(TAG, "cacheDir=" + cacheDir);
        QuadNative.setDataPath(filesDir);
        QuadNative.setTempDir(cacheDir);
        QuadNative.setDpi(getResources().getDisplayMetrics().densityDpi);

        // 2.5 将 APK assets 中的内置资源包复制到可写目录，供 Rust 侧解压
        copyBuiltinAssets(filesDir);

        // 3. 初始化输入框后端。
        QuadNative.initializeEnvironment();

        // 3.5 处理深链接（phira://）启动参数，须在 activityOnCreate 前调用。
        handleDeepLink(getIntent());

        // 3.6 处理"打开方式"传入的文件（导入谱面/资源包）。
        handleOpenFile(getIntent());

        // 4. 设置 SurfaceView 并交给原生侧渲染。
        view = new QuadSurface(this);
        layout = new ResizingLayout(this);
        layout.addView(view);
        setContentView(layout);

        // 5. 触发 Rust 侧 quad_main()（内部会启动渲染线程并立即返回）。
        QuadNative.activityOnCreate(this);

        // 6. 强制全屏（隐藏状态栏/导航栏）。
        applyFullscreen();
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        // 每次窗口获得焦点时重新应用全屏，避免系统栏重新出现
        if (hasFocus) {
            applyFullscreen();
        }
    }

    /** 强制沉浸式全屏（隐藏状态栏与导航栏）。 */
    private void applyFullscreen() {
        View decorView = getWindow().getDecorView();
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = decorView.getWindowInsetsController();
            if (controller != null) {
                controller.hide(WindowInsets.Type.systemBars());
                controller.setSystemBarsBehavior(
                        WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        } else {
            decorView.setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    | View.SYSTEM_UI_FLAG_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_STABLE);
        }
    }

    /** 解析 phira:// 深链接，把多人房间参数传给原生侧。 */
    private void handleDeepLink(Intent intent) {
        if (intent == null) {
            return;
        }
        Uri uri = intent.getData();
        if (uri == null || !"phira".equals(uri.getScheme())) {
            return;
        }
        // 用 host + path + query 组装，避免 getSchemeSpecificPart() 的 "//" 前缀导致匹配失败
        String host = uri.getHost();   // room
        String path = uri.getPath();   // /join/123456
        String q = uri.getQuery();     // server=xxx
        StringBuilder raw = new StringBuilder();
        if (host != null) {
            raw.append(host);
        }
        if (path != null) {
            raw.append(path);
        }
        if (q != null && q.length() > 0) {
            raw.append('&').append(q);
        }

        String join = null;
        String create = null;
        String server = null;
        for (String part : raw.toString().split("[&?]")) {
            if (part.startsWith("room/join/")) {
                join = part.substring("room/join/".length());
            } else if (part.startsWith("room/create/")) {
                create = part.substring("room/create/".length());
            } else if (part.startsWith("server=")) {
                server = part.substring("server=".length());
            }
        }
        if (join != null || create != null || server != null) {
            Log.i(TAG, "deep link: join=" + join + " create=" + create + " server=" + server);
            // 原生侧用空字符串表示“未提供”，避免跨 JNI 传递 null
            QuadNative.setStartupArgs(join == null ? "" : join, create == null ? "" : create, server == null ? "" : server);
        }
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        handleDeepLink(intent);
        handleOpenFile(intent);
    }

    /** 处理"打开方式"传入的文件（导入谱面/资源包，Rust 自动识别）。 */
    private void handleOpenFile(Intent intent) {
        if (intent == null) {
            return;
        }
        String action = intent.getAction();
        Uri uri = null;
        if (Intent.ACTION_VIEW.equals(action)) {
            uri = intent.getData();
        } else if (Intent.ACTION_SEND.equals(action)) {
            uri = intent.getParcelableExtra(Intent.EXTRA_STREAM);
            if (uri == null && intent.getClipData() != null && intent.getClipData().getItemCount() > 0) {
                // 部分文件管理器只填 ClipData、不带 EXTRA_STREAM
                uri = intent.getClipData().getItemAt(0).getUri();
            }
        } else if (Intent.ACTION_SEND_MULTIPLE.equals(action)) {
            // 多文件分享：取第一个文件导入
            if (intent.getClipData() != null && intent.getClipData().getItemCount() > 0) {
                uri = intent.getClipData().getItemAt(0).getUri();
            } else {
                List<Uri> uris = intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM);
                if (uris != null && !uris.isEmpty()) {
                    uri = uris.get(0);
                }
            }
        }
        if (uri == null) {
            return;
        }
        String cachePath = copyToCache(uri);
        if (cachePath != null) {
            // 自动识别谱面/资源包（Rust 端根据 zip 内容判断）
            QuadNative.markAutoImport();
            QuadNative.setChosenFile(cachePath);
            Log.i(TAG, "open file import: " + cachePath);
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        QuadNative.activityOnResume();
        QuadNative.prprActivityOnResume();
    }

    @Override
    protected void onPause() {
        super.onPause();
        QuadNative.activityOnPause();
        QuadNative.prprActivityOnPause();
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        QuadNative.activityOnDestroy();
        QuadNative.prprActivityOnDestroy();
        QuadNative.releaseContext();
        instance = null;
    }

    // ------------------------------------------------------------------
    // 由原生侧（.so）通过 JNI 调用的方法 —— 方法名与签名不可随意更改
    // ------------------------------------------------------------------

    /** 原生侧请求选择一个文件（用于导入谱面 / 资源包 / 头像等）。 */
    public void chooseFile() {
        runOnUiThread(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType("*/*");
                intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, false);
                startActivityForResult(Intent.createChooser(intent, "选择文件"), REQ_OPEN_FILE);
            } catch (Exception e) {
                Log.e(TAG, "chooseFile failed", e);
            }
        });
    }

    /** 原生侧请求选择一个文件夹（用于导入自定义资源目录等）。
     *  SAF 返回的是 content:// 树 Uri，Rust 侧无法直接访问，
     *  因此在 onActivityResult 中将整个文件夹递归复制到 filesDir，
     *  再把本地路径通过 setChosenFile 回调给原生侧。 */
    public void chooseFolder() {
        runOnUiThread(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
                intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                startActivityForResult(intent, REQ_OPEN_FOLDER);
            } catch (Exception e) {
                Log.e(TAG, "chooseFolder failed", e);
            }
        });
    }

    /** 原生侧请求导出文件（SAF 创建文档），suggestedName 为建议文件名。 */
    public void showExportDialog(String suggestedName) {
        runOnUiThread(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType("*/*");
                intent.putExtra(Intent.EXTRA_TITLE, suggestedName == null ? "phira_export" : suggestedName);
                startActivityForResult(Intent.createChooser(intent, "导出文件"), REQ_CREATE_FILE);
            } catch (Exception e) {
                Log.e(TAG, "showExportDialog failed", e);
            }
        });
    }

    /** 删除 SAF 导出的 Uri（导出完成后由原生侧通过 deleter 回调触发）。 */
    public void deleteUri(Uri uri) {
        try {
            getContentResolver().delete(uri, null, null);
        } catch (Exception e) {
            Log.e(TAG, "deleteUri failed", e);
        }
    }

    /** 打开外部链接。 */
    public void openUrl(String url) {
        runOnUiThread(() -> {
            try {
                Intent i = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
                i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                startActivity(i);
            } catch (Exception e) {
                Log.e(TAG, "openUrl failed: " + url, e);
            }
        });
    }

    /** 防沉迷回调（未启用 aa feature 时为空实现）。 */
    public void antiAddiction(String action, String arg) {
        Log.i(TAG, "antiAddiction: action=" + action + " arg=" + arg);
    }

    /** 复制文本到剪贴板（miniquad clipboard_set）。 */
    public void copy(String text) {
        ClipboardManager cm = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);
        if (cm != null && text != null) {
            cm.setPrimaryClip(ClipData.newPlainText("text", text));
        }
    }

    /** 全屏切换（miniquad setFullscreen）。 */
    public void setFullScreen(final boolean fullscreen) {
        runOnUiThread(() -> {
            View decorView = getWindow().getDecorView();
            if (fullscreen) {
                getWindow().setFlags(
                        WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
                        WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS);
                if (Build.VERSION.SDK_INT >= 28) {
                    WindowManager.LayoutParams lp = getWindow().getAttributes();
                    lp.layoutInDisplayCutoutMode =
                            WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
                    getWindow().setAttributes(lp);
                }
                if (Build.VERSION.SDK_INT >= 30) {
                    getWindow().setDecorFitsSystemWindows(false);
                } else {
                    int uiOptions = View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY;
                    decorView.setSystemUiVisibility(uiOptions);
                }
            } else {
                if (Build.VERSION.SDK_INT >= 30) {
                    getWindow().setDecorFitsSystemWindows(true);
                } else {
                    decorView.setSystemUiVisibility(0);
                }
            }
        });
    }

    /** 软键盘显示/隐藏（miniquad showKeyboard）。 */
    public void showKeyboard(final boolean show) {
        runOnUiThread(() -> {
            try {
                InputMethodManager imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
                if (imm == null) {
                    Log.w(TAG, "showKeyboard: no InputMethodManager");
                    return;
                }
                if (show) {
                    // 输入框激活时禁用 ResizingLayout 的 IME padding，避免游戏画面被压缩
                    if (layout != null) {
                        layout.setImeResizeEnabled(false);
                    }
                    WindowManager.LayoutParams attrs = getWindow().getAttributes();
                    attrs.softInputMode = WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE
                            | WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE;
                    getWindow().setAttributes(attrs);
                    if (view != null) {
                        view.setFocusable(true);
                        view.setFocusableInTouchMode(true);
                        view.requestFocusFromTouch();
                        view.requestFocus();
                    }
                    // 延迟显示软键盘，确保焦点/IME 已就绪
                    getWindow().getDecorView().postDelayed(() -> {
                        if (view == null) {
                            return;
                        }
                        boolean ok = imm.showSoftInput(view, InputMethodManager.SHOW_IMPLICIT);
                        if (!ok) {
                            imm.showSoftInput(view, InputMethodManager.SHOW_FORCED);
                        }
                    }, 150);
                } else {
                    if (view != null) {
                        imm.hideSoftInputFromWindow(view.getWindowToken(), 0);
                        imm.restartInput(view);
                        view.clearFocus();
                    }
                    view.postDelayed(() -> {
                        if (view == null) {
                            return;
                        }
                        view.setFocusable(true);
                        view.setFocusableInTouchMode(true);
                        // 恢复 IME padding 和 adjustResize，让其他控件跟随键盘
                        WindowManager.LayoutParams attrs = getWindow().getAttributes();
                        attrs.softInputMode = WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE;
                        getWindow().setAttributes(attrs);
                        if (layout != null) {
                            layout.setImeResizeEnabled(true);
                        }
                    }, 300);
                }
            } catch (Exception e) {
                Log.e(TAG, "showKeyboard failed show=" + show, e);
            }
        });
    }

    // ------------------------------------------------------------------
    // SAF 文件选择 / 导出结果处理
    // ------------------------------------------------------------------

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (resultCode != RESULT_OK || data == null || data.getData() == null) {
            return;
        }
        Uri uri = data.getData();
        if (requestCode == REQ_OPEN_FILE) {
            String cachePath = copyToCache(uri);
            if (cachePath != null) {
                // 原生侧根据 request_file 传入的 id（_import / _import_respack 等）自动判断导入类型。
                QuadNative.setChosenFile(cachePath);
            }
        } else if (requestCode == REQ_OPEN_FOLDER) {
            // SAF 文件夹选择：递归复制到 filesDir，再回调本地路径。
            String folderPath = copyFolderToFilesDir(uri);
            if (folderPath != null) {
                QuadNative.setChosenFile(folderPath);
            }
        } else if (requestCode == REQ_CREATE_FILE) {
            try {
                ParcelFileDescriptor pfd = getContentResolver().openFileDescriptor(uri, "w");
                if (pfd != null) {
                    // 把 fd 的所有权移交给原生侧，由原生侧写入并关闭、最终 deleteUri。
                    QuadNative.processExportFd(uri, pfd.detachFd());
                }
            } catch (Exception e) {
                Log.e(TAG, "export failed", e);
            }
        }
    }

    /** 把 APK assets 中的内置资源包复制到 filesDir/assets/，供 Rust 侧 std::fs 读取和解压。 */
    private void copyBuiltinAssets(String filesDir) {
        File assetsDir = new File(filesDir, "assets");
        if (!assetsDir.exists()) {
            //noinspection ResultOfMethodCallIgnored
            assetsDir.mkdirs();
        }
        String[] packages = {"Expansion_package.zip", "Ontology_package.zip"};
        for (String name : packages) {
            File target = new File(assetsDir, name);
            if (target.exists()) {
                Log.i(TAG, "asset already exists: " + name);
                continue;
            }
            InputStream in = null;
            OutputStream out = null;
            try {
                in = getAssets().open(name);
                out = new FileOutputStream(target);
                byte[] buf = new byte[64 * 1024];
                int n;
                while ((n = in.read(buf)) > 0) {
                    out.write(buf, 0, n);
                }
                out.flush();
                Log.i(TAG, "copied asset: " + name + " (" + target.length() + " bytes)");
            } catch (IOException e) {
                Log.e(TAG, "failed to copy asset: " + name, e);
            } finally {
                try {
                    if (in != null) in.close();
                    if (out != null) out.close();
                } catch (IOException ignored) {
                }
            }
        }
    }

    /** 把 SAF 返回的 content Uri 复制到应用缓存目录，返回本地绝对路径。 */
    private String copyToCache(Uri uri) {
        InputStream in = null;
        OutputStream out = null;
        try {
            String displayName = queryDisplayName(uri);
            File dir = new File(getCacheDir(), "imports");
            if (!dir.exists()) {
                //noinspection ResultOfMethodCallIgnored
                dir.mkdirs();
            }
            File target = new File(dir, UUID.randomUUID() + "_" + sanitize(displayName));
            in = getContentResolver().openInputStream(uri);
            if (in == null) {
                return null;
            }
            out = new FileOutputStream(target);
            byte[] buf = new byte[64 * 1024];
            int n;
            while ((n = in.read(buf)) > 0) {
                out.write(buf, 0, n);
            }
            out.flush();
            return target.getAbsolutePath();
        } catch (IOException e) {
            Log.e(TAG, "copyToCache failed", e);
            return null;
        } finally {
            try {
                if (in != null) {
                    in.close();
                }
                if (out != null) {
                    out.close();
                }
            } catch (IOException ignored) {
            }
        }
    }

    // ------------------------------------------------------------------
    // SAF 文件夹选择：递归复制到应用私有目录
    // ------------------------------------------------------------------

    /** 把 SAF ACTION_OPEN_DOCUMENT_TREE 返回的树 Uri 递归复制到 filesDir，返回本地目录路径。 */
    private String copyFolderToFilesDir(Uri treeUri) {
        try {
            String folderName = queryFolderName(treeUri);
            File targetRoot = new File(getFilesDir(), "folder_imports");
            if (!targetRoot.exists()) {
                //noinspection ResultOfMethodCallIgnored
                targetRoot.mkdirs();
            }
            File targetDir = new File(targetRoot, UUID.randomUUID() + "_" + sanitize(folderName));
            if (!targetDir.exists()) {
                //noinspection ResultOfMethodCallIgnored
                targetDir.mkdirs();
            }
            copyDocumentTree(treeUri, targetDir);
            Log.i(TAG, "folder imported: " + targetDir.getAbsolutePath());
            return targetDir.getAbsolutePath();
        } catch (Exception e) {
            Log.e(TAG, "copyFolderToFilesDir failed", e);
            return null;
        }
    }

    /** 递归遍历 SAF 文档树，把所有文件/子目录复制到 targetDir。 */
    private void copyDocumentTree(Uri treeUri, File targetDir) {
        try {
            String docId = DocumentsContract.getDocumentId(treeUri);
            Uri childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, docId);
            android.database.Cursor c = getContentResolver().query(
                    childrenUri,
                    new String[]{
                            DocumentsContract.Document.COLUMN_MIME_TYPE,
                            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                            DocumentsContract.Document.COLUMN_DOCUMENT_ID
                    },
                    null, null, null);
            if (c == null) {
                return;
            }
            try {
                while (c.moveToNext()) {
                    String mime = c.getString(0);
                    String name = c.getString(1);
                    String childDocId = c.getString(2);
                    Uri childUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, childDocId);
                    String safeName = sanitize(name);
                    if (DocumentsContract.Document.MIME_TYPE_DIR.equals(mime)) {
                        File subDir = new File(targetDir, safeName);
                        if (!subDir.exists()) {
                            //noinspection ResultOfMethodCallIgnored
                            subDir.mkdirs();
                        }
                        copyDocumentTree(childUri, subDir);
                    } else {
                        File targetFile = new File(targetDir, safeName);
                        copyUriToFile(childUri, targetFile);
                    }
                }
            } finally {
                c.close();
            }
        } catch (Exception e) {
            Log.e(TAG, "copyDocumentTree failed", e);
        }
    }

    /** 把单个 content Uri 复制到目标文件。 */
    private void copyUriToFile(Uri uri, File target) {
        InputStream in = null;
        OutputStream out = null;
        try {
            in = getContentResolver().openInputStream(uri);
            if (in == null) {
                return;
            }
            out = new FileOutputStream(target);
            byte[] buf = new byte[64 * 1024];
            int n;
            while ((n = in.read(buf)) > 0) {
                out.write(buf, 0, n);
            }
            out.flush();
        } catch (IOException e) {
            Log.e(TAG, "copyUriToFile failed: " + target, e);
        } finally {
            try {
                if (in != null) in.close();
                if (out != null) out.close();
            } catch (IOException ignored) {
            }
        }
    }

    /** 查询 SAF 树 Uri 对应的文件夹显示名称。 */
    private String queryFolderName(Uri treeUri) {
        android.database.Cursor c = null;
        try {
            String docId = DocumentsContract.getDocumentId(treeUri);
            Uri docUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, docId);
            c = getContentResolver().query(
                    docUri,
                    new String[]{DocumentsContract.Document.COLUMN_DISPLAY_NAME},
                    null, null, null);
            if (c != null && c.moveToFirst()) {
                String name = c.getString(0);
                if (name != null && !name.isEmpty()) {
                    return name;
                }
            }
        } catch (Exception ignored) {
        } finally {
            if (c != null) {
                c.close();
            }
        }
        return "folder";
    }

    private String queryDisplayName(Uri uri) {
        android.database.Cursor c = null;
        try {
            c = getContentResolver().query(uri, null, null, null, null);
            if (c != null && c.moveToFirst()) {
                int idx = c.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME);
                if (idx >= 0) {
                    return c.getString(idx);
                }
            }
        } catch (Exception ignored) {
        } finally {
            if (c != null) {
                c.close();
            }
        }
        return "file";
    }

    private static String sanitize(String name) {
        if (name == null) {
            return "file";
        }
        return name.replaceAll("[^A-Za-z0-9._-]", "_");
    }

    /** 检查应用是否拥有存储读取权限（Android 6 及以下使用运行时权限时）。 */
    public boolean hasStoragePermission() {
        if (Build.VERSION.SDK_INT >= 23) {
            return checkSelfPermission(android.Manifest.permission.READ_EXTERNAL_STORAGE)
                    == PackageManager.PERMISSION_GRANTED;
        }
        return true;
    }
}
