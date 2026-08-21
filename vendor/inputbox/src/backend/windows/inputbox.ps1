Add-Type -AssemblyName System.Windows.Forms;
Add-Type -AssemblyName System.Drawing;

$jsonString = $args[0];
if ([string]::IsNullOrWhiteSpace($jsonString)) {
    $stdin = [System.Console]::OpenStandardInput()
    $reader = New-Object System.IO.StreamReader($stdin,[System.Text.Encoding]::UTF8)
    $jsonString = $reader.ReadToEnd()
    $reader.Close()
}

if ([string]::IsNullOrWhiteSpace($jsonString)) {
    Write-Error "No JSON input provided.";
    exit 1;
}

$config = $jsonString | ConvertFrom-Json;

# 主题配色
$bgColor = [System.Drawing.Color]::FromArgb(28, 31, 38);
$fgColor = [System.Drawing.Color]::FromArgb(228, 231, 237);
$inputBg = [System.Drawing.Color]::FromArgb(22, 25, 31);
$accent = [System.Drawing.Color]::FromArgb(33, 150, 243);
$danger = [System.Drawing.Color]::FromArgb(120, 128, 140);
$font = New-Object System.Drawing.Font("Segoe UI", 10);
$fontSmall = New-Object System.Drawing.Font("Segoe UI", 9);

$form = New-Object System.Windows.Forms.Form;
$form.Text = $config.title;
$form.BackColor = $bgColor;
$form.ForeColor = $fgColor;
$form.Font = $font;
$form.StartPosition = 'CenterScreen';
$form.FormBorderStyle = 'FixedDialog';
$form.MaximizeBox = $false;
$form.MinimizeBox = $false;
$form.TopMost = $true;

$label = New-Object System.Windows.Forms.Label;
$label.Text = $config.prompt;
$label.ForeColor = $fgColor;
$label.Location = New-Object System.Drawing.Point(20, 18);
$label.AutoSize = $true;
$label.MaximumSize = New-Object System.Drawing.Size(360, 0);
$form.Controls.Add($label);

$form.PerformLayout();
$labelBottom = $label.Location.Y + $label.Height;

$textBox = New-Object System.Windows.Forms.TextBox;
$textBox.Location = New-Object System.Drawing.Point(20, ($labelBottom + 12));
$textBox.Text = $config.default;
$textBox.BackColor = $inputBg;
$textBox.ForeColor = $fgColor;
$textBox.BorderStyle = 'FixedSingle';
$textBox.Font = $font;

if ($config.mode -eq "multiline") {
    $textBox.Multiline = $true;
    $textBox.Size = New-Object System.Drawing.Size(360, 150);
    $textBox.WordWrap = $config.auto_wrap;

    if ($textBox.WordWrap) {
        $textBox.ScrollBars = 'Vertical';
    } else {
        $textBox.ScrollBars = 'Both';
    }

    $textBox.AcceptsReturn = $true;
} else {
    $textBox.Multiline = $false;
    $textBox.Size = New-Object System.Drawing.Size(360, 30);

    if ($config.mode -eq "password") {
        $textBox.UseSystemPasswordChar = $true;
    }
}
$form.Controls.Add($textBox);

$textBoxBottom = $textBox.Location.Y + $textBox.Height;

function New-ThemedButton {
    param($Text, $X, $Y, $Back, $Fore, $DialogResult)
    $btn = New-Object System.Windows.Forms.Button;
    $btn.Size = New-Object System.Drawing.Size(88, 32);
    $btn.Location = New-Object System.Drawing.Point($X, $Y);
    $btn.Text = $Text;
    $btn.BackColor = $Back;
    $btn.ForeColor = $Fore;
    $btn.FlatStyle = 'Flat';
    $btn.FlatAppearance.BorderSize = 0;
    $btn.Font = $fontSmall;
    $btn.Cursor = 'Hand';
    $btn.DialogResult = $DialogResult;
    return $btn;
}

$btnY = $textBoxBottom + 16;

$cancelButton = New-ThemedButton -Text $config.cancel_label -X 192 -Y $btnY -Back $danger -Fore ([System.Drawing.Color]::White) -DialogResult ([System.Windows.Forms.DialogResult]::Cancel);
$form.Controls.Add($cancelButton);

$okButton = New-ThemedButton -Text $config.ok_label -X 292 -Y $btnY -Back $accent -Fore ([System.Drawing.Color]::White) -DialogResult ([System.Windows.Forms.DialogResult]::OK);
$form.Controls.Add($okButton);

$form.CancelButton = $cancelButton;

if ($config.mode -ne "multiline") {
    $form.AcceptButton = $okButton;
}

$form.ClientSize = New-Object System.Drawing.Size(400, ($cancelButton.Location.Y + $cancelButton.Height + 16));

if ($config.mode -eq "multiline") {
    $textBox.Anchor = 'Top, Bottom, Left, Right';
} else {
    $textBox.Anchor = 'Top, Left, Right';
}

$cancelButton.Anchor = 'Bottom, Right';
$okButton.Anchor = 'Bottom, Right';

$size = $form.ClientSize;
if ($null -ne $config.width) {
    $size.Width = [int]$config.width;
}
if ($null -ne $config.height) {
    $size.Height = [int]$config.height;
}
$form.ClientSize = $size;

$form.Add_Shown({
    $textBox.Select();

    $scrollToEnd = $config.scroll_to_end;

    if ($scrollToEnd) {
        $textBox.SelectionStart = $textBox.Text.Length;
        $textBox.SelectionLength = 0;
        $textBox.ScrollToCaret();
    } else {
        $textBox.SelectionStart = 0;
        $textBox.SelectionLength = 0;
    }
});

$result = $form.ShowDialog();

if ($result -eq[System.Windows.Forms.DialogResult]::OK) {
    $res = $textBox.Text;
    if ($null -ne $res) {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($res);
        $stdout =[System.Console]::OpenStandardOutput();
        $stdout.Write($bytes, 0, $bytes.Length);
        $stdout.Flush();
    }
} else {
    exit 1;
}
