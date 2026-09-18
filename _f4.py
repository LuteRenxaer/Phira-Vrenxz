# -*- coding: utf-8 -*-
p = r'F:\phirLie\Phira-Vrenxz\src\mp\page\room.rs'
raw = open(p,'rb').read(); crlf = b'\r\n' in raw
d = open(p,'r',encoding='utf-8',newline='').read().replace('\r\n','\n')

def rep(old, new, label):
    global d
    assert old in d, label
    d = d.replace(old, new, 1)
    print('OK:', label)

# 房名/谱面名位置：靠顶部
rep('''        theme::text_left_bold(ui, left_rect.x + pad, body_top + FS_HERO * 0.62, FS_HERO, Color::new(0.12, 0.12, 0.16, 1.), &title, left_rect.w - pad * 2.);
        let (state_text, chart_name) = chart_parts(room, ctx.view);
        let sub = chart_name.clone().unwrap_or_else(|| mtl!("mp-state-choose").into_owned());
        theme::text_left(ui, left_rect.x + pad, body_top + FS_HERO * 0.62 + FS_HERO * 0.7, FS_SUB, Color::new(0.25, 0.25, 0.3, 1.), &format!("谱面: {}", sub), left_rect.w - pad * 2.);''',
'''        let (state_text, chart_name) = chart_parts(room, ctx.view);
        theme::text_left_bold(ui, left_rect.x + pad, body_top + FS_HERO * 0.5, FS_HERO, Color::new(0.12, 0.12, 0.16, 1.), &title, left_rect.w - pad * 2.);
        let sub = chart_name.clone().unwrap_or_else(|| mtl!("mp-state-choose").into_owned());
        theme::text_left(ui, left_rect.x + pad, body_top + FS_HERO * 0.5 + FS_SUB * 1.0, FS_SUB, Color::new(0.25, 0.25, 0.3, 1.), &format!("谱面: {}", sub), left_rect.w - pad * 2.);''',
'title pos')

# 左下卡片：压扁，贴底但不压按钮
rep('''        let card_h = 0.5 * SCALE;
        let card_w = left_rect.w * 0.62;
        let card = Rect::new(left_rect.x + pad, body_bottom - pad - card_h, card_w, card_h);''',
'''        let card_h = 0.30 * SCALE;
        let card_w = left_rect.w * 0.58;
        let card = Rect::new(left_rect.x + pad, body_bottom - pad - card_h, card_w, card_h);''',
'card size')

# 底部小按钮：在卡片上方一行
rep('''        let mut fy = card.bottom() + 0.03;
        let mut fx = left_rect.x + pad;''',
'''        let mut fy = card.y - 0.11;
        let mut fx = left_rect.x + pad;''',
'small btns pos')

out = d.replace('\n','\r\n') if crlf else d
open(p,'w',encoding='utf-8',newline='').write(out)
print('PASS')
