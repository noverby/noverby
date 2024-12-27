import React, { Suspense } from 'react';
import { ChangeList, ContentApp, PollList, FolderDial, CommentList } from 'comps';
import { Node } from 'hooks';
import { Stack } from '@mui/material';

const PolicyApp = ({ node }: { node: Node }) => (
  <Stack spacing={1}>
    <ContentApp node={node} />
    <Suspense fallback={null}>
      <CommentList node={node} />
    </Suspense>
    <Suspense fallback={null}>
      <ChangeList node={node} />
    </Suspense>
    <PollList node={node} />
    <Suspense fallback={null}>
      <FolderDial node={node} />
    </Suspense>
  </Stack>
);

export default PolicyApp;
