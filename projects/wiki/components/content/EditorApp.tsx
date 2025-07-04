import { Editor } from "comps";
import type { Node } from "hooks";
import { Suspense } from "react";

const EditorApp = ({ node }: { node: Node }) => (
	<Suspense fallback={null}>
		<Editor node={node} />
	</Suspense>
);

export default EditorApp;
