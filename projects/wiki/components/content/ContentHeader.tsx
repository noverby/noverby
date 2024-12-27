import { CardHeader, Skeleton, Tooltip, Typography } from '@mui/material';
import { MimeAvatarNode, MemberChips, ContentToolbar, MimeAvatar } from 'comps';
import { Suspense } from 'react';
import { Node, useScreen } from 'hooks';
import { Stack } from '@mui/system';
import formatDistance from 'date-fns/formatDistance';
import { da } from 'date-fns/locale';

const Title = ({ node }: { node: Node }) => {
  const query = node.useQuery();
  const screen = useScreen();
  return query?.name ? (
    <Stack>
      <Typography variant={screen ? 'h5' : 'body1'} sx={{ color: 'inherit' }}>
        {query?.name}
      </Typography>
    </Stack>
  ) : null;
};

const Subtitle = ({ node }: { node: Node }) => {
  const query = node.useQuery();

  return <Tooltip
    title={
      query?.createdAt && new Date(query?.createdAt).toLocaleString()
    }
  >
    <Typography component='span' variant="caption">
      {query?.createdAt ? `
                Oprettet
                ${formatDistance(new Date(), new Date(query?.createdAt), {
        locale: da,
      })
        } siden
            ` : ""}
    </Typography>
  </Tooltip>
};

const ContentHeader = ({
  node,
  hideMembers,
  child,
  add,
}: {
  node: Node;
  hideMembers?: boolean;
  child?: boolean;
  add?: boolean;
}) => {
  return (
    <>
      <CardHeader
        title={
          child ? (
            'Mappe'
          ) : (
            <Suspense fallback={<Skeleton width={10} />}>
              <Title node={node} />
            </Suspense>
          )
        }
        subheader={
          child ? undefined : <Suspense fallback={null} ><Subtitle node={node} /></Suspense>
        }
        avatar={
          child ? (
            <MimeAvatar mimeId="app/folder" />
          ) : (
            <MimeAvatarNode node={node} />
          )
        }
        sx={{
          borderRadius: '4px 4px 0px 0px',
        }}
        action={<Suspense fallback={null}><ContentToolbar child={child} add={add} node={node} /></Suspense>}
      />
      {!hideMembers && <Suspense fallback={null}><MemberChips node={node} /></Suspense>}
    </>
  );
};

export default ContentHeader;
