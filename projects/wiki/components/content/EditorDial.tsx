import React, { useState } from 'react';
import { Avatar, Box, Fab } from '@mui/material';
import { SpeedDial, SpeedDialAction } from '@mui/material';
import { Save, Public } from '@mui/icons-material';
import { resolve } from 'gql';
import { Node, useLink, useScreen, useSession } from 'hooks';

const checkIfSuperParent = async (
  id?: string,
  superParentId?: string
): Promise<boolean> => {
  if (!(id && superParentId)) return false;
  const parentId = await resolve(({ query }) => query.node({ id })?.parentId);

  return id == superParentId || parentId === superParentId
    ? true
    : parentId === null
    ? false
    : checkIfSuperParent(parentId!, superParentId);
};

const EditorDial = ({
  node,
  handleSave,
}: {
  node: Node;
  handleSave: Function;
}) => {
  const screen = useScreen();
  const [session, setSession] = useSession();
  const [open, setOpen] = useState(false);
  const [openDelete, setOpenDelete] = useState(false);
  const link = useLink();
  const query = node.useQuery();
  const id = query?.id;
  const nodeInsert = node.useInsert();
  const nodeUpdate = node.useUpdate();
  const nodeMembers = node.useMembers();

  const editable = query?.mutable && query?.isOwner;
  return (
    <>
      <Box display="flex" justifyContent="flex-end">
        {editable ? (
          <SpeedDial
            ariaLabel="Gem eller indsend"
            sx={{
              bottom: (t) => t.spacing(10),
              position: 'absolute',
            }}
            icon={<Save />}
            onOpen={() => setOpen(true)}
            onClose={() => setOpen(false)}
            open={open}
          >
            <SpeedDialAction
              icon={
                <Avatar sx={{ bgcolor: 'primary.main' }}>{<Save />}</Avatar>
              }
              tooltipTitle="Gem"
              tooltipOpen
              onClick={() => handleSave(false)}
            />
            <SpeedDialAction
              icon={
                <Avatar sx={{ bgcolor: 'primary.main' }}>{<Public />}</Avatar>
              }
              tooltipTitle="Indsend"
              tooltipOpen
              onClick={() => handleSave(true)}
            />
          </SpeedDial>
        ) : (
          <Fab
            sx={{
              bottom: (t) => t.spacing(10),
              position: 'absolute',
              bgcolor: 'primary.main',
              color: (t) => t.palette.primary.contrastText,
            }}
            onClick={() => handleSave(true)}
          >
            <Save />
          </Fab>
        )}
      </Box>
    </>
  );
};

export default EditorDial;
