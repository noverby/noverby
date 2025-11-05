import { ContentApp } from "comps";
import type { Node } from "hooks";

const CandidateApp = ({ node }: { node: Node }) => (
	<ContentApp node={node} hideMembers />
);

export default CandidateApp;
