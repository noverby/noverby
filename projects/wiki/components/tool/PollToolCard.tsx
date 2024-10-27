import { CardActions, Card, CardHeader } from '@mui/material';
import { Poll } from '@mui/icons-material';
import { Node } from 'hooks';
import { AutoButton, PollDialog } from 'comps';
import { useState } from 'react';
import { IconId } from 'mime';

const PollToolCard = ({ node }: { node: Node }) => {
  const query = node.useQuery();
  const inserts = query?.inserts()?.map((mime) => mime.id);
  const [openPollDialog, setOpenPollDialog] = useState(false);

  const handleAddPoll = () => {
    setOpenPollDialog(true);
  };
  return (
    <>
      {inserts?.includes('vote/poll') && (
        <>
          <PollDialog
            key="poll-dialog"
            node={node}
            open={openPollDialog}
            setOpen={setOpenPollDialog}
          />
          <Card elevation={0}>
            <CardHeader
              avatar={<IconId mimeId="app/vote" />}
              title="Afstemning"
            />
            <CardActions>
              <AutoButton
                key="poll"
                text="Ny afstemning"
                icon={<Poll />}
                onClick={handleAddPoll}
              />
            </CardActions>
          </Card>
        </>
      )}
    </>
  );
};

export default PollToolCard;
