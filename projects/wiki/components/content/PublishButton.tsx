import React, { useState } from 'react';
import {
  Dialog,
  DialogTitle,
  DialogActions,
  Button,
  DialogContent,
} from '@mui/material';
import { Node } from 'hooks';
import { Publish } from '@mui/icons-material';
import { AutoButton } from 'components/common';

const PublishButton = ({
  node,
  handlePublish,
}: {
  node: Node;
  handlePublish?: () => Promise<void>;
}) => {
  const [open, setOpen] = useState(false);
  const update = node.useUpdate();
  const query = node.useQuery();

  const handler =
    handlePublish ??
    (async () => {
      update({ set: { mutable: false } });
    });

  if (!query?.mutable) return null;

  return (
    <>
      <AutoButton
        key="sent"
        text="Indsend"
        icon={<Publish />}
        onClick={() => setOpen(true)}
      />
      <Dialog open={open} onClose={() => setOpen(false)}>
        <DialogTitle>Bekræft Indsendelse</DialogTitle>
        <DialogContent>
          Når du har indsendt, så er det ikke muligt at redigere mere.
        </DialogContent>
        <DialogActions>
          <Button
            endIcon={<Publish />}
            variant="contained"
            color="primary"
            onClick={async () => {
              await handler();
              setOpen(false);
            }}
          >
            Indsend
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
};

export default PublishButton;
