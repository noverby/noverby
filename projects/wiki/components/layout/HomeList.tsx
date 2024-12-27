import { Event, EventBusy, Group, GroupRemove } from '@mui/icons-material';
import {
  Avatar,
  List,
  ListItem,
  ListItemAvatar,
  ListItemButton,
  ListItemText,
  Typography,
  lighten,
} from '@mui/material';
import { useUserId } from '@nhost/nextjs';
import { fromId } from 'core/path';
import { order_by, resolve, useQuery } from 'gql';
import { useLink, useSession } from 'hooks';
import { Fragment, startTransition } from 'react';

const abriv: { [name: string]: string } = {
  Hovedbestyrelsesmøde: 'HB',
  Landsmøde: 'LM',
};

const abrivContextName = (name?: string) => {
  const split = name
    ?.trim()
    .split(' ')
    .filter(
      (name) =>
        (name[0] === name[0].toUpperCase() &&
          !(/[0-9]/.test(name) && name.length > 1)) ||
        (name.match(/[A-Z]/g)?.length ?? 1) > 1
    )
    .map((name) => (abriv[name] ? abriv[name] : (name.match(/[A-Z]/g)?.length ?? 1) > 1 ? name : name[0]));

  switch (split?.length) {
    case 1:
      return split[0];
    case 2:
      return split[0] + split[1];
    case 3:
      return split[0] + split[1] + split[2];
  }
};

const groupBy = <T,>(list: Array<T>, keyGetter: (item: T) => string) => {
  const map = new Map<string, T[]>();
  list.forEach((item) => {
    const key = keyGetter(item);
    const collection = map.get(key);
    if (!collection) {
      map.set(key, [item]);
    } else {
      // eslint-disable-next-line functional/immutable-data
      collection.push(item);
    }
  });
  return map;
};

/**
 * Convert a string to a color using a hash function
 * @param str string to convert
 * @returns hex color in the format #xxxxxx
 */
const stringToRgb = (str: string) => {
  const i = str
    .split('')
    .reduce((hash, _, i) => str.charCodeAt(i) + ((hash << 5) - hash), 0);
  const hash = (i & 0x00ffffff).toString(16).toUpperCase();

  return '#00000'.substring(1, 6 - hash.length) + hash;
};

const HomeList = ({ setOpen }: { setOpen?: Function }) => {
  const link = useLink();
  const userId = useUserId();
  const query = useQuery();
  const [_, setSession] = useSession();
  const events = !userId
    ? []
    : query.nodes({
      order_by: [{ createdAt: order_by.desc }],
      where: {
        _and: [
          { mimeId: { _eq: 'wiki/event' } },
          {
            _or: [
              { ownerId: { _eq: userId } },
              {
                members: {
                  _and: [
                    { accepted: { _eq: true } },
                    { nodeId: { _eq: userId } },
                  ],
                },
              },
            ],
          },
        ],
      },
    });
  const eventByYears = groupBy(
    events,
    (event) => event.createdAt?.substring(0, 4)!
  );
  const groups = !userId
    ? []
    : query.nodes({
      order_by: [{ createdAt: order_by.desc }],
      where: {
        _and: [
          { mimeId: { _eq: 'wiki/group' } },
          {
            _or: [
              { ownerId: { _eq: userId } },
              {
                members: {
                  _and: [
                    { accepted: { _eq: true } },
                    { nodeId: { _eq: userId } },
                  ],
                },
              },
            ],
          },
        ],
      },
    });

  const handleContextSelect = (id: string) => async () => {
    const prefix = await resolve(({ query }) => {
      const node = query.node({ id });
      return {
        id: node?.id,
        name: node?.name ?? '',
        mime: node?.mimeId!,
        key: node?.key,
      };
    });

    const path = await fromId(id);
    startTransition(() => {
      setSession({
        prefix: {
          ...prefix,
          path,
        },
      });
      setOpen?.(false);
      link.id(id);
    });
  };

  return (
    <>
      <List>
        <ListItem key={-1}>
          <ListItemAvatar>
            <Avatar sx={{ bgcolor: 'secondary.main' }}>
              <Group />
            </Avatar>
          </ListItemAvatar>
          <ListItemText primary="Grupper" />
        </ListItem>
        {groups.map(({ id = '0', name, key }) => {
          const item = (
            <ListItemButton
              key={id}
              hidden={id == '0'}
              dense
              onClick={handleContextSelect(id)}
            >
              <ListItemAvatar>
                {
                  <Avatar
                    sx={{
                      width: 35,
                      height: 35,
                      bgcolor: (t) => lighten(t.palette.primary.main, 0.3),
                    }}
                  >
                    <Typography fontSize={15}>
                      {abrivContextName(name)}
                    </Typography>{' '}
                  </Avatar>
                }
              </ListItemAvatar>
              <ListItemText primary={name} />
            </ListItemButton>
          );
          return id ? item : null;
        })}
        {!groups?.[0]?.id && (
          <ListItemButton key={-2}>
            <ListItem>
              <ListItemAvatar>
                <Avatar sx={{ width: 35, height: 35 }}>
                  <GroupRemove />
                </Avatar>
              </ListItemAvatar>
              <ListItemText primary="Ingen grupper" />
            </ListItem>
          </ListItemButton>
        )}
      </List>
      <List>
        <ListItem key={-1}>
          <ListItemAvatar>
            <Avatar sx={{ bgcolor: 'secondary.main' }}>
              <Event />
            </Avatar>
          </ListItemAvatar>
          <ListItemText primary="Begivenheder" />
        </ListItem>
        {[...eventByYears.entries()].map(([year, events]) => {
          return (
            <Fragment key={year ?? 0}>
              <ListItem>
                <ListItemText
                  primary={
                    <Typography sx={{ fontWeight: 'bold' }}>{year}</Typography>
                  }
                />
              </ListItem>

              {events.map(({ id = '0', name, key }) => {
                const item = (
                  <ListItemButton
                    key={id}
                    hidden={id == '0'}
                    dense
                    onClick={handleContextSelect(id)}
                  >
                    <ListItemAvatar>
                      {
                        <Avatar
                          sx={{
                            width: 35,
                            height: 35,
                            bgcolor: (t) =>
                              lighten(t.palette.primary.main, 0.3),
                          }}
                        >
                          <Typography fontSize={15}>
                            {abrivContextName(name)}
                          </Typography>{' '}
                        </Avatar>
                      }
                    </ListItemAvatar>
                    <ListItemText primary={name} />
                  </ListItemButton>
                );
                return id ? item : null;
              })}
            </Fragment>
          );
        })}

        {!events?.[0]?.id && (
          <ListItemButton key={-2}>
            <ListItem>
              <ListItemAvatar>
                <Avatar sx={{ width: 35, height: 35 }}>
                  <EventBusy />
                </Avatar>
              </ListItemAvatar>
              <ListItemText primary="Ingen begivenheder" />
            </ListItem>
          </ListItemButton>
        )}
      </List>
    </>
  );
};

export default HomeList;
