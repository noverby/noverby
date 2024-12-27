import React, { useState } from 'react';
import { AddContentDialog, AutoButton } from 'comps';
import { PlusOne } from '@mui/icons-material';
import { Node } from 'hooks';

const AddQuestionButton = ({ node }: { node: Node }) => {
  const [open, setOpen] = useState(false);
  const query = node.useQuery();

  const handleSubmit = async () => {
    setOpen(true);
  };

  if (!query?.inserts()?.some((mime) => mime.id == 'vote/question'))
    return null;

  return (
    <>
      <AutoButton text="Spørgsmål" icon={<PlusOne />} onClick={handleSubmit} />
      <AddContentDialog
        mutable={false}
        node={node}
        mimes={['vote/question']}
        open={open}
        setOpen={setOpen}
      />
    </>
  );
};

export default AddQuestionButton;
