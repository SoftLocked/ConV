import { StateEffect, StateField } from '@codemirror/state';
import { Decoration, EditorView, WidgetType } from '@codemirror/view';
export type Cursor = { position: number; name: string; color: string };
export const cursors = StateEffect.define<Cursor[]>();
class Label extends WidgetType {
  constructor(private cursor: Cursor) { super(); }
  toDOM() {
    const node = document.createElement('span'); node.className = 'remote-cursor';
    node.style.borderColor = this.cursor.color;
    node.contentEditable = 'false'; node.setAttribute('aria-hidden', 'true');
    node.dataset.name = this.cursor.name;
    node.style.setProperty('--cursor-color', this.cursor.color);
    return node;
  }
}
export const presence = StateField.define({
  create: () => Decoration.none,
  update(value, transaction) {
    value = value.map(transaction.changes);
    for (const effect of transaction.effects) if (effect.is(cursors)) {
      value = Decoration.set(effect.value.map(cursor => Decoration.widget({ widget: new Label(cursor), side: 1 }).range(Math.min(cursor.position, transaction.newDoc.length))), true);
    }
    return value;
  },
  provide: field => EditorView.decorations.from(field),
});
