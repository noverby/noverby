import React, { startTransition, useEffect } from 'react';
import { Fragment, useState } from 'react';
import { SortFab, MimeAvatarId } from 'comps';
import {
  Card,
  List,
  ListItem,
  ListItemAvatar,
  ListItemText,
} from '@mui/material';
import { nodes, order_by, resolve } from 'gql';
import {
  DragDropContext,
  Droppable,
  Draggable,
  OnDragEndResponder,
} from 'react-beautiful-dnd';
import { Node } from 'hooks';

const SortApp = ({ node }: { node: Node }) => {
  const [list, setList] = useState<Partial<nodes>[]>([]);

  useEffect(() => {
    const fetch = async () => {
      const children = await resolve(
        ({ query }) =>
          query.node({ id: node.id! })
            ?.children({
              order_by: [{ index: order_by.asc }],
              where: { mime: { hidden: { _eq: false } } },
            })
            .map(({ id, name, index, mutable, mimeId, data }) => ({
              id,
              name,
              index,
              mutable,
              mimeId,
              data,
            })) ?? [],
        { cachePolicy: "no-cache" }
      );
      setList(children);
    };
    startTransition(() => {
      fetch();
    });
  }, []);

  const handleDragEnd: OnDragEndResponder = ({ source, destination }) => {
    if (destination === undefined || destination === null) return;
    const newList = list.filter((_, index) => index !== source.index);
    const res = [
      ...newList.slice(0, destination.index),
      list[source.index],
      ...newList.slice(destination.index),
    ];
    setList(res);
  };

  return (
    <>
      <DragDropContext onDragEnd={handleDragEnd}>
        <Card sx={{ m: 0 }}>
          <Droppable droppableId="drop1">
            {(provided, snapshot) => (
              <List ref={provided.innerRef} sx={{ m: 0 }}>
                {list?.[0]?.id &&
                  list.map((node, index: number) => (
                    <Draggable
                      key={node.id}
                      draggableId={node.id!}
                      index={index}
                    >
                      {(provided, snapshot) => (
                        <Fragment>
                          <ListItem
                            component="li"
                            ref={provided.innerRef}
                            {...provided.draggableProps}
                            {...provided.dragHandleProps}
                          >
                            {node.id && (
                              <ListItemAvatar>
                                <MimeAvatarId id={node.id} />
                              </ListItemAvatar>
                            )}
                            <ListItemText primary={node.name} />
                          </ListItem>
                        </Fragment>
                      )}
                    </Draggable>
                  ))}
                {provided.placeholder}
              </List>
            )}
          </Droppable>
        </Card>
      </DragDropContext>
      <SortFab node={node} elements={list} />
    </>
  );
};

export default SortApp;
