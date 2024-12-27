import { Suspense } from 'react';
import { Editor } from 'comps';
import { Node } from 'hooks';

const EditorApp = ({ node }: { node: Node }) => (
  <Suspense fallback={null}>
    <Editor node={node} />
  </Suspense>
);

export default EditorApp;
