ObjC.import("AppKit");

var PADDING = 20;
var GAP = 8;
var BUTTON_WIDTH = 90;
var BUTTON_HEIGHT = 32;
var LABEL_FONT_SIZE = 13;
var TITLE_FONT_SIZE = 15;

function toStr(v) {
  return v == null ? "" : String(v);
}
function ns(v) {
  return $(toStr(v));
}

function readStdin() {
  var handle = $.NSFileHandle.fileHandleWithStandardInput;
  var data = handle.readDataToEndOfFile;
  var str = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding);
  return ObjC.unwrap(str);
}

function installEditMenu() {
  var mainMenu = $.NSMenu.alloc.init;
  var editMenuItem = $.NSMenuItem.alloc.initWithTitleActionKeyEquivalent(
    ns("Edit"),
    null,
    ns(""),
  );
  var editMenu = $.NSMenu.alloc.initWithTitle(ns("Edit"));
  var items = [
    { title: "Select All", action: "selectAll:", key: "a" },
    { title: "Copy", action: "copy:", key: "c" },
    { title: "Paste", action: "paste:", key: "v" },
    { title: "Cut", action: "cut:", key: "x" },
    { title: "Undo", action: "undo:", key: "z" },
  ];
  for (var i = 0; i < items.length; i++) {
    editMenu.addItem(
      $.NSMenuItem.alloc.initWithTitleActionKeyEquivalent(
        ns(items[i].title),
        items[i].action,
        ns(items[i].key),
      ),
    );
  }
  editMenuItem.submenu = editMenu;
  mainMenu.addItem(editMenuItem);
  $.NSApplication.sharedApplication.mainMenu = mainMenu;
}

function makeLabel(text, fontSize, bold, width) {
  var label = $.NSTextField.alloc.initWithFrame($.NSMakeRect(0, 0, width, 20));
  label.stringValue = ns(text);
  label.editable = false;
  label.bordered = false;
  label.drawsBackground = false;
  label.selectable = true;
  label.lineBreakMode = $.NSLineBreakByWordWrapping;
  label.font = bold
    ? $.NSFont.boldSystemFontOfSize(fontSize)
    : $.NSFont.systemFontOfSize(fontSize);
  label.preferredMaxLayoutWidth = width;
  label.sizeToFit;
  return label;
}

function buildDialog(opts) {
  var winW = opts.width || 520;
  var contentW = winW - PADDING * 2;
  var inputH = opts.height || (opts.mode === "multiline" ? 180 : 24);

  var inputBundle;
  if (opts.mode === "password") {
    var field = $.NSSecureTextField.alloc.initWithFrame(
      $.NSMakeRect(0, 0, contentW, 24),
    );
    field.stringValue = ns(opts.defaultText);
    field.font = $.NSFont.systemFontOfSize(LABEL_FONT_SIZE);
    inputBundle = {
      view: field,
      focusView: field,
      readValue: function () {
        return ObjC.unwrap(field.stringValue);
      },
    };
  } else if (opts.mode === "multiline") {
    var scrollView = $.NSScrollView.alloc.initWithFrame(
      $.NSMakeRect(0, 0, contentW, inputH),
    );
    scrollView.hasVerticalScroller = true;
    scrollView.hasHorizontalScroller = !opts.auto_wrap;
    scrollView.autohidesScrollers = true;
    scrollView.borderType = $.NSBezelBorder;

    var textView = $.NSTextView.alloc.initWithFrame(scrollView.contentSize);
    textView.verticallyResizable = true;
    textView.horizontallyResizable = !opts.auto_wrap;
    textView.font = $.NSFont.systemFontOfSize(LABEL_FONT_SIZE);
    textView.string = ns(opts.defaultText);

    if (opts.auto_wrap) {
      textView.textContainer.widthTracksTextView = true;
      textView.textContainer.containerSize = $.NSMakeSize(
        scrollView.contentSize.width,
        Number.MAX_VALUE,
      );
    } else {
      textView.textContainer.widthTracksTextView = false;
      textView.textContainer.containerSize = $.NSMakeSize(
        Number.MAX_VALUE,
        Number.MAX_VALUE,
      );
    }

    if (opts.scroll_to_end) {
      var len = textView.string.length;
      textView.setSelectedRange($.NSMakeRange(len, 0));
      textView.scrollRangeToVisible($.NSMakeRange(len, 0));
    }
    scrollView.documentView = textView;
    inputBundle = {
      view: scrollView,
      focusView: textView,
      readValue: function () {
        return ObjC.unwrap(textView.string);
      },
    };
  } else {
    var field = $.NSTextField.alloc.initWithFrame(
      $.NSMakeRect(0, 0, contentW, 24),
    );
    field.stringValue = ns(opts.defaultText);
    field.font = $.NSFont.systemFontOfSize(LABEL_FONT_SIZE);
    inputBundle = {
      view: field,
      focusView: field,
      readValue: function () {
        return ObjC.unwrap(field.stringValue);
      },
    };
  }

  var titleLabel = opts.title
    ? makeLabel(opts.title, TITLE_FONT_SIZE, true, contentW)
    : null;
  var promptLabel = opts.prompt
    ? makeLabel(opts.prompt, LABEL_FONT_SIZE, false, contentW)
    : null;

  var y = PADDING;
  var btnY = y;
  y += BUTTON_HEIGHT + GAP * 2;
  var inputY = y;
  y += inputBundle.view.frame.size.height;
  var promptY = y + GAP;
  if (promptLabel) y += promptLabel.frame.size.height + GAP;
  var titleY = y + GAP;
  if (titleLabel) y += titleLabel.frame.size.height + GAP;
  y += PADDING;

  var win = $.NSWindow.alloc.initWithContentRectStyleMaskBackingDefer(
    $.NSMakeRect(0, 0, winW, y),
    $.NSTitledWindowMask | $.NSClosableWindowMask,
    $.NSBackingStoreBuffered,
    false,
  );

  var cv = win.contentView;
  if (titleLabel) {
    titleLabel.setFrameOrigin($.NSMakePoint(PADDING, titleY));
    cv.addSubview(titleLabel);
  }
  if (promptLabel) {
    promptLabel.setFrameOrigin($.NSMakePoint(PADDING, promptY));
    cv.addSubview(promptLabel);
  }
  inputBundle.view.setFrameOrigin($.NSMakePoint(PADDING, inputY));
  cv.addSubview(inputBundle.view);

  var okBtn = $.NSButton.alloc.initWithFrame(
    $.NSMakeRect(
      winW - PADDING - BUTTON_WIDTH,
      btnY,
      BUTTON_WIDTH,
      BUTTON_HEIGHT,
    ),
  );
  okBtn.title = ns(opts.ok_label);
  okBtn.bezelStyle = $.NSBezelStyleRounded;
  okBtn.keyEquivalent = ns("\r");
  if (opts.mode === "multiline")
    okBtn.keyEquivalentModifierMask = $.NSEventModifierFlagCommand;

  var cancelBtn = $.NSButton.alloc.initWithFrame(
    $.NSMakeRect(
      winW - PADDING - BUTTON_WIDTH * 2 - 8,
      btnY,
      BUTTON_WIDTH,
      BUTTON_HEIGHT,
    ),
  );
  cancelBtn.title = ns(opts.cancel_label);
  cancelBtn.bezelStyle = $.NSBezelStyleRounded;
  cancelBtn.keyEquivalent = ns("\u001b");

  cv.addSubview(okBtn);
  cv.addSubview(cancelBtn);

  return {
    window: win,
    inputBundle: inputBundle,
    okButton: okBtn,
    cancelButton: cancelBtn,
  };
}

function runDialog(dialog) {
  var app = $.NSApplication.sharedApplication;
  ObjC.registerSubclass({
    name: "DialogDelegate",
    methods: {
      "onOK:": {
        types: ["void", ["id"]],
        implementation: function () {
          app.stopModalWithCode(1);
        },
      },
      "onCancel:": {
        types: ["void", ["id"]],
        implementation: function () {
          app.stopModalWithCode(0);
        },
      },
    },
  });
  var delegate = $.DialogDelegate.alloc.init;
  dialog.okButton.target = delegate;
  dialog.okButton.action = "onOK:";
  dialog.cancelButton.target = delegate;
  dialog.cancelButton.action = "onCancel:";

  app.setActivationPolicy($.NSApplicationActivationPolicyRegular);
  dialog.window.center;
  dialog.window.makeKeyAndOrderFront(null);
  app.activateIgnoringOtherApps(true);
  dialog.window.makeFirstResponder(dialog.inputBundle.focusView);

  var result = app.runModalForWindow(dialog.window);
  dialog.window.orderOut(null);

  if (result === 0) throw new Error("cancelled");
  return dialog.inputBundle.readValue();
}

function run() {
  var rawInput = readStdin();
  if (!rawInput) return;

  var input = JSON.parse(rawInput);
  installEditMenu();

  var dialog = buildDialog(input);
  return runDialog(dialog);
}
