import React from 'react';
import { InvitesTextField, InvitesFab, MembersDataGrid } from 'comps';
import { Stack } from '@mui/material';
import { Node } from 'hooks';

const MemberApp = ({ node }: { node: Node }) => {
  if (!node?.id) return null;

  return (
    <>
      <Stack spacing={1}>
        <InvitesTextField node={node} />
        <MembersDataGrid node={node} />
      </Stack>
      <InvitesFab node={node} />
    </>
  );
}

export default MemberApp;