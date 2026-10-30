import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';

/** Ordinary desktop, read-only scan results; selection is exercised without preparing deletion. */
export async function checkPurposeResults({ command, session, js, click, until, ipc, output }) {
  const originalTheme = (await ipc('bootstrap')).settings.theme;
  const themes = { light: '浅色', dark: '深色', system: '跟随系统' };
  async function theme(value) {
    await click('//nav/button[normalize-space(.)="设置"]');
    await click(`//button[normalize-space(.)="${themes[value]}"]`);
    await until(async () => (await ipc('bootstrap')).settings.theme === value);
    await click('//nav/button[normalize-space(.)="清理"]');
  }
  const layouts = [];
  const key = (value) =>
    command('POST', `/session/${session}/actions`, {
      actions: [
        {
          type: 'key',
          id: 'keyboard',
          actions: [
            { type: 'keyDown', value },
            { type: 'keyUp', value },
          ],
        },
      ],
    });
  try {
    await until(() =>
      js(
        'return !!document.querySelector(".recommended-results .purpose-heading input:not(:disabled)");',
      ),
    );
    assert.equal(await js('return !!document.querySelector(".cleanup-adjustments");'), false);
    for (const mode of ['light', 'dark']) {
      await theme(mode);
      for (const [width, height] of [
        [760, 580],
        [1100, 780],
      ]) {
        await command('POST', `/session/${session}/window/rect`, { width, height });
        const layout = await until(() =>
          js(`
          const main=document.querySelector('.main-content'); main.scrollTop=0;
          const buttons=[...document.querySelectorAll('button')].filter(e=>e.textContent.trim().startsWith('清理已选') && e.getBoundingClientRect().width>0);
          if(buttons.length!==1)return null;
          const rect=buttons[0].getBoundingClientRect();
          const rescans=[...document.querySelectorAll('button')].filter(e=>e.textContent.trim()==='重新全面扫描' && e.getBoundingClientRect().width>0);
          const rescan=rescans[0]?.getBoundingClientRect();
          return {singleAction:buttons.length===1,actionVisible:rect.top>=0&&rect.bottom<=innerHeight,rescanVisible:rescans.length===1&&rescan.top>=0&&rescan.bottom<=innerHeight,overflow:main.scrollWidth>main.clientWidth+1,firstChoiceVisible:document.querySelector('.recommended-results .purpose-heading').getBoundingClientRect().bottom<=main.getBoundingClientRect().bottom};`),
        );
        assert(
          layout.singleAction && layout.actionVisible && layout.rescanVisible && !layout.overflow,
          JSON.stringify(layout),
        );
        await writeFile(
          `${output}/purpose-${mode}-${width}.png`,
          Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
        );
        // A persistent partial-scan notice may push choices below the fold. Keep
        // the notice and verify that the focused choice can be reached unobscured.
        const choice = await js(`
          const input=document.querySelector('.recommended-results .purpose-heading input');
          input.focus(); input.scrollIntoView({block:'center',behavior:'instant'});
          const rect=input.getBoundingClientRect(), main=document.querySelector('.main-content').getBoundingClientRect();
          const hit=document.elementFromPoint(rect.x+rect.width/2,rect.y+rect.height/2);
          return {focused:document.activeElement===input,visible:rect.top>=main.top&&rect.bottom<=main.bottom,unobscured:hit===input};`);
        assert(choice.focused && choice.visible && choice.unobscured, JSON.stringify(choice));
        layouts.push({ mode, width, height, ...layout, scrolledChoice: choice });
        await writeFile(
          `${output}/purpose-${mode}-${width}-choice.png`,
          Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
        );
      }
    }
    const button = await js(
      'return document.querySelector(".recommended-results .purpose-toggle").getAttribute("aria-label");',
    );
    const checked = await js(
      'const c=document.querySelector(".recommended-results .purpose-heading input"); c.focus(); return c.checked;',
    );
    await key(' ');
    await until(() =>
      js(
        `return document.querySelector('.recommended-results .purpose-heading input').checked === ${!checked};`,
      ),
    );
    await key(' ');
    await until(() =>
      js(
        `return document.querySelector('.recommended-results .purpose-heading input').checked === ${checked};`,
      ),
    );
    await click(`//button[@aria-label="${button}"]`);
    const detail = await js(
      'return document.querySelector(".recommended-results .purpose-group .details-button").getAttribute("aria-label");',
    );
    await click(`//button[@aria-label="${detail}"]`);
    await until(() => js('return !!document.querySelector(".file-row input");'));
    await key('\uE00C');
    await until(() => js('return document.activeElement?.classList.contains("details-button");'));
    return {
      layouts,
      firstLevelSelection: true,
      keyboardSelection: true,
      escapeClosesDetail: true,
      detailFocusRestored: true,
      noDeletion: true,
    };
  } finally {
    await theme(originalTheme);
  }
}
