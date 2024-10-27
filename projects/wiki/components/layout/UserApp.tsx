import React from 'react';
import {
  List,
  ListItem,
  ListItemText,
  ListItemAvatar,
  Avatar,
  ListItemButton,
} from '@mui/material';
import { HeaderCard, Link } from 'comps';
import { useQuery } from 'gql';
import { Event, Group, Subject } from '@mui/icons-material';
import { IconId } from 'mime';
import { Node } from 'hooks';

const UserApp = ({ node }: { node: Node }) => {
  const query = useQuery();
  const nodes = query.nodes({
    where: {
      _or: [{ ownerId: { _eq: node?.id } }],
    },
  });
  return (
    <>
      <HeaderCard title="Medlemskaber" avatar={<Group />}>
        <List>
          {query
            ?.members({
              where: {
                _and: [
                  { nodeId: { _eq: node?.id } },
                  { parent: { mimeId: { _eq: 'wiki/group' } } },
                ],
              },
            })
            .map(({ parent, node }) => (
              <ListItemButton
                key={node?.id ?? 0}
                component={Link}
                href={parent?.id ?? ''}
              >
                <ListItemText primary={parent?.name} />
              </ListItemButton>
            )) ?? (
            <ListItem>
              <ListItemText primary="Ingen medlemskaber" />
            </ListItem>
          )}
        </List>
      </HeaderCard>
      <HeaderCard title="Begivenheder" avatar={<Event />}>
        <List>
          {query
            ?.members({
              where: {
                _and: [
                  { nodeId: { _eq: node?.id } },
                  { parent: { mimeId: { _eq: 'wiki/event' } } },
                ],
              },
            })
            .map(({ parent, node }) => (
              <ListItemButton
                key={node?.id ?? 0}
                component={Link}
                href={parent?.id ?? ''}
              >
                <ListItemText primary={parent?.name} />
              </ListItemButton>
            )) ?? (
            <ListItem>
              <ListItemText primary="Ingen medlemskaber" />
            </ListItem>
          )}
        </List>
      </HeaderCard>
      <HeaderCard title="Indhold" avatar={<Subject />}>
        <List>
          {nodes?.map(({ id, name, mimeId, parent }) => (
            <ListItemButton key={id} component={Link} href={id ?? ''}>
              <ListItemAvatar>
                <Avatar
                  sx={{
                    bgcolor: 'secondary.main',
                  }}
                >
                  <IconId mimeId={mimeId} />
                </Avatar>
              </ListItemAvatar>
              <ListItemText primary={name} secondary={parent?.name} />
            </ListItemButton>
          )) ?? (
            <ListItem>
              <ListItemText primary="Intet indhold" />
            </ListItem>
          )}
        </List>
      </HeaderCard>
    </>
  );
}

export default UserApp;