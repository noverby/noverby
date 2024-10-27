import { ButtonGroup, Stack } from '@mui/material';
import { Edit, People } from '@mui/icons-material';
import { Node, useLink } from 'hooks';
import {
  AutoButton,
  DeleteButton,
  DownloadButton,
  AddContentButton,
  PublishButton,
} from 'comps';

const ContentToolbar = ({
  node,
  child,
  add,
}: {
  node: Node;
  child?: boolean;
  add?: boolean;
}) => {
  const query = node.useQuery();
  const link = useLink();

  return (
    <Stack spacing={1} direction="row">
      {!child &&
        ['wiki/event', 'wiki/group'].includes(query?.mimeId ?? '') &&
        query?.isContextOwner && (
          <AutoButton
            key="member"
            text="Medlemmer"
            icon={<People />}
            onClick={() => link.push([], 'member')}
          />
        )}
      {!child && <DownloadButton node={node} />}
      {!child && (
        <ButtonGroup>
          {((query?.mutable && query?.isOwner) || query?.isContextOwner) && [
            <DeleteButton key="delete" node={node} />,
            <AutoButton
              key="edit"
              text="Rediger"
              icon={<Edit />}
              onClick={() => link.push([], 'editor')}
            />,
            <PublishButton key="publish" node={node} />,
          ]}
        </ButtonGroup>
      )}
      {add && <AddContentButton node={node} />}
    </Stack>
  );
};

export default ContentToolbar;
