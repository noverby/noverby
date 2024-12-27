import React from 'react';
import { ContentApp } from 'comps';
import { Node } from 'hooks';

const CandidateApp = ({ node }: { node: Node }) => (
  <ContentApp node={node} hideMembers />
);

export default CandidateApp;
