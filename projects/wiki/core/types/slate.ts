// This example is for an Editor with `ReactEditor` and `HistoryEditor`
import { BaseEditor } from 'slate';
import { ReactEditor } from 'slate-react';
import { HistoryEditor } from 'slate-history';

export type CustomEditor = BaseEditor & ReactEditor & HistoryEditor;

export const STYLE_TYPES = [
  'paragraph',
  'heading-one',
  'heading-two',
  'heading-three',
  'heading-four',
  'heading-five',
  'heading-six',
  'block-quote',
  'block-pre',
] as const;
export const LIST_TYPES = ['numbered-list', 'bulleted-list', 'list-item'] as const;
export const TEXT_ALIGN_TYPES = ['left', 'center', 'right', 'justify'] as const;
export const OTHER_TYPES = ['link', 'image'] as const;

export type StyleType = (typeof STYLE_TYPES)[number];
export type ListType = (typeof LIST_TYPES)[number];
export type TextAlignType = (typeof TEXT_ALIGN_TYPES)[number];
export type OtherType = (typeof OTHER_TYPES)[number];
export type ElementType = StyleType | ListType | TextAlignType | OtherType;

export type CustomElement = {
  [key: string]: ElementType | CustomText[] | TextAlignType | string | null | undefined;
  type: ElementType;
  children?: CustomText[];
  align?: TextAlignType;
  url?: string | null;
};
export type CustomText = {
  [key: string]: string | boolean | undefined;
  text: string;
  code?: boolean;
  strikethrough?: boolean;
  italic?: boolean;
  bold?: boolean;
  underline?: boolean;
  link?: string;
};

declare module 'slate' {
  // eslint-disable-next-line @typescript-eslint/consistent-type-definitions
  interface CustomTypes {
    Editor: CustomEditor;
    Element: CustomElement;
    Text: CustomText;
  }
}
