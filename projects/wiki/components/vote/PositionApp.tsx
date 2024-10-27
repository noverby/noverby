import React from 'react';
import {
  ContentApp,
  PollList,
  CandidateList,
  FolderDial,
  QuestionList,
} from 'comps';
import { Node } from 'hooks';
import { Stack } from '@mui/material';

const PositionApp = ({ node }: { node: Node }) => (
  <Stack spacing={1}>
    <ContentApp node={node} add />
    <CandidateList node={node} />
    <QuestionList node={node} />
    <PollList node={node} />
    <FolderDial node={node} />
  </Stack>
);

export default PositionApp;
