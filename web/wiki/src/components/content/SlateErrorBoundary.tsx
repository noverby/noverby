import React, { type ReactNode } from "react";
import { type Editor, Transforms } from "slate";

export default class SlateErrorBoundary extends React.Component<{
	children: ReactNode;
	editor: Editor;
}> {
	static getDerivedStateFromError() {
		return {};
	}

	componentDidCatch() {
		Transforms.deselect(this.props.editor);
	}

	render() {
		return this.props.children;
	}
}
