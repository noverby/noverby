import React from 'react';
import { AddCommentButton, AutoButton } from 'comps';
import { LowPriority, Face, DoNotDisturb, Delete } from '@mui/icons-material';
import {
  Avatar,
  Collapse,
  Card,
  CardHeader,
  IconButton,
  List,
  ListItem,
  ListItemAvatar,
  ListItemSecondaryAction,
  ListItemText,
  CardActions,
  Typography,
  Chip,
  ListItemButton,
} from '@mui/material';
import { order_by } from 'gql';
import { IconId } from 'mime';
import { Node, useLink, useScreen } from 'hooks';
import { TransitionGroup } from 'react-transition-group';
import { useUserId } from '@nhost/nextjs';

const CommentList = ({ node }: { node: Node }) => {
  const screen = useScreen();
  const link = useLink();
  const query = node.useQuery();
  const $delete = node.useDelete();
  const userId = useUserId();
  const children = query?.children({
    where: { mimeId: { _eq: 'vote/comment' } },
    order_by: [{ index: order_by.asc }],
  });

  return (
    <Card sx={{ m: 0 }}>
      <CardHeader
        title={<Typography>Kommentarer</Typography>}
        avatar={
          <Avatar
            sx={{
              bgcolor: 'secondary.main',
            }}
          >
            <IconId mimeId="vote/comment" />
          </Avatar>
        }
        action={
          !screen && (
            <CardActions sx={{ p: 0 }}>
              {query?.isContextOwner && !screen && (
                <AutoButton
                  text="Sorter"
                  icon={<LowPriority />}
                  onClick={() => link.push([], 'sort')}
                />
              )}
              <AddCommentButton node={node} />
            </CardActions>
          )
        }
      />
      <List>
        <TransitionGroup>
          {children?.map(({ id, data, owner, isOwner }, index) => {
            const item = (
              <Collapse key={id ?? 0}>
                <ListItem>
                  <ListItemAvatar>
                    <Avatar
                      sx={{
                        bgcolor: 'secondary.main',
                      }}
                    >
                      {index + 1}
                    </Avatar>
                  </ListItemAvatar>
                  <ListItemText
                    primary={data()?.text}
                    secondary={
                      <Chip
                        key={owner?.id}
                        icon={<Face />}
                        color="secondary"
                        variant="outlined"
                        size="small"
                        sx={{ mr: 0.5 }}
                        label={owner?.displayName}
                      />
                    }
                  />
                  {!screen && (
                    <ListItemSecondaryAction>
                      <IconButton
                        color="primary"
                        onClick={() => {
                          $delete({ id });
                        }}
                        size="large"
                      >
                        <Delete />
                      </IconButton>
                    </ListItemSecondaryAction>
                  )}
                </ListItem>
              </Collapse>
            );
            return (isOwner || query?.isContextOwner) && id ? item : null;
          })}
          {!children?.[0]?.id && (
            <Collapse key={-1}>
              <ListItemButton>
                <ListItemAvatar>
                  <Avatar
                    sx={{
                      bgcolor: 'secondary.main',
                    }}
                  >
                    <DoNotDisturb />
                  </Avatar>
                </ListItemAvatar>
                <ListItemText primary="Ingen kommentarer" />
              </ListItemButton>
            </Collapse>
          )}
        </TransitionGroup>
      </List>
    </Card>
  );
};

export default CommentList;
