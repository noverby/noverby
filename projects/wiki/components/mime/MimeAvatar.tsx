import { LockOpen } from '@mui/icons-material';
import { Badge, Tooltip, Avatar as MuiAvatar, Skeleton } from '@mui/material';
import { Maybe } from 'gql';
import { withSuspense } from 'hoc';
import { Node, useNode, useScreen } from 'hooks';
import { IconId } from 'mime';

const MimeAvatar = ({
  mimeId,
  index,
  name,
}: {
  mimeId: Maybe<string | undefined>;
  index?: number;
  name?: string;
}) => {
  const screen = useScreen();
  return (
    <MuiAvatar
      sx={{
        bgcolor: 'secondary.main',
      }}
    >
      <IconId name={name} mimeId={mimeId} index={index} avatar />
    </MuiAvatar>
  );
};

const Avatar = ({ node }: { node: Node }) => {
  const query = node?.useQuery();
  const type = query?.data?.({ path: 'type' });
  const mimeId = query?.mimeId;
  const id = type ?? mimeId;
  const name = query?.name;
  const index = query?.getIndex;

  if (id === undefined) {
    return (
      <MuiAvatar
        sx={{
          bgcolor: 'secondary.main',
        }}
      >
        {' '}
      </MuiAvatar>
    );
  }

  const avatar = (
    <MuiAvatar
      sx={{
        bgcolor: 'secondary.main',
      }}
    >
      <IconId name={name} mimeId={id} index={index ? index - 1 : undefined} avatar />
    </MuiAvatar>
  );
  return query?.mutable ? (
    <Badge
      overlap="circular"
      anchorOrigin={{
        vertical: 'bottom',
        horizontal: 'right',
      }}
      badgeContent={
        <Tooltip title="Ikke indsendt">
          <MuiAvatar
            sx={{
              width: 18,
              height: 18,
              bgcolor: 'primary.main',
            }}
          >
            <LockOpen
              sx={{
                width: 14,
                height: 14,
                color: '#fff',
              }}
            />
          </MuiAvatar>
        </Tooltip>
      }
    >
      {avatar}
    </Badge>
  ) : (
    avatar
  );
};

const MimeAvatarNode = withSuspense(Avatar, <Skeleton variant="circular" width={24} height={24} /> );
const MimeAvatarId = withSuspense(({ id, ...props }: { id: string }) => {
  const node = useNode({ id });
  return <Avatar node={node} {...props} />;
}, <Skeleton variant="circular" width={24} height={24} />  );

export { MimeAvatar, MimeAvatarNode, MimeAvatarId };
