import React, { useState } from 'react';
import { Dialog, DialogTitle, DialogActions, Button } from '@mui/material';
import { Node, useLink } from 'hooks';
import { Delete } from '@mui/icons-material';
import { AutoButton } from 'comps';

const DeleteButton = ({ node }: { node: Node }) => {
  const [open, setOpen] = useState(false);
  const link = useLink();
  const $delete = node.useDelete();
  const members = node.useMembers();

  const handleDelete = async () => {
    await members.delete();
    await $delete();
    link.pop();
  };

  return (
    <>
      <AutoButton
        key="delete"
        text="Slet"
        icon={<Delete />}
        onClick={() => setOpen(true)}
      />
      <Dialog open={open} onClose={() => setOpen(false)}>
        <DialogTitle>Bekræft Sletning</DialogTitle>
        <DialogActions>
          <Button
            endIcon={<Delete />}
            variant="contained"
            color="primary"
            onClick={handleDelete}
          >
            Slet
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
};

export default DeleteButton;
