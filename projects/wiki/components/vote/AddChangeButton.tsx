import React, { useState } from 'react';
import { AddContentDialog, AutoButton } from 'comps';
import { PlusOne } from '@mui/icons-material';
import { Node } from 'hooks';
import { useUserDisplayName } from '@nhost/nextjs';

const AddChangeButton = ({ node }: { node: Node }) => {
  const displayName = useUserDisplayName();
  const [open, setOpen] = useState(false);
  const query = node.useQuery();

  const name = query?.mimeId == 'vote/position' ? displayName : '';

  const handleSubmit = () => {
    setOpen(true);
  };

  if (!query?.inserts()?.some((mime) => mime.id == 'vote/change')) return null;

  return (
    <>
      <AutoButton
        text="Nyt Ændringsforslag"
        icon={<PlusOne />}
        onClick={handleSubmit}
      />
      <AddContentDialog
        initTitel={name}
        node={node}
        mimes={['vote/change']}
        open={open}
        setOpen={setOpen}
        redirect
        app="editor"
      />
    </>
  );
};

export default AddChangeButton;
