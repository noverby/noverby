import { Stack } from '@mui/material';
import { FolderApp, ContentApp } from 'comps';
import { Node } from 'hooks';

const EventApp = ({ node }: { node: Node }) => (
  <Stack spacing={1}>
    <ContentApp node={node} hideMembers />
    <FolderApp node={node} child />
  </Stack>
);

export default EventApp;
