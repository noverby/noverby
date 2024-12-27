import React, { useState } from 'react';
import {
  Dialog,
  DialogTitle,
  DialogActions,
  DialogContent,
  Button,
  FormControlLabel,
  Switch,
  Slider,
  Stack,
  Typography,
} from '@mui/material';
import { Node, useLink, useSession } from 'hooks';
import { PlayArrow } from '@mui/icons-material';

const PollDialog = ({
  node,
  open,
  setOpen,
}: {
  node: Node;
  open: boolean;
  setOpen: Function;
}) => {
  const link = useLink();
  const [session] = useSession();
  const [loading, setLoading] = useState(false);
  const get = node.useSubsGet();
  const pollId = get('active')?.id;
  const insert = node.useInsert();
  const update = node.useUpdate();
  const query = node.useQuery();
  const nodeId = query?.id;
  const context = node.useContext();
  const contextSet = context.useSet();

  const [hidden, setHidden] = useState(query?.mimeId == 'vote/position');
  const [voteCount, setVoteCount] = React.useState<number[]>([1, 1]);

  const handleChange = (event: Event, newValue: number | number[]) => {
    setVoteCount(newValue as number[]);
  };

  const options = ['vote/policy', 'vote/change'].includes(query?.mimeId ?? '')
    ? ['For', 'Imod', 'Blank']
    : query
        ?.children({ where: { mimeId: { _eq: 'vote/candidate' } } })
        .map(({ name }) => name)
        .concat('Blank');
  const optionsCount = options?.length || 0;

  const handleAddPoll = async () => {
    setLoading(true);
    if (pollId) await update({ id: pollId, set: { mutable: false } });
    const key = new Date(new Date().getTime() + (session?.timeDiff ?? 0))
      .toLocaleString()
      .replaceAll('/', '');
    const poll = await insert({
      name: query?.name,
      key,
      mimeId: 'vote/poll',
      data: {
        minVote: voteCount[0],
        maxVote: voteCount[1],
        hidden,
        options: options,
        nodeId,
      },
    });

    await contextSet('active', poll.id ?? null);
    link.push([poll.key!])
    setLoading(false);
  };

  const getMarks = (count: number) =>
    [...Array(count - 1).keys()].map((i) => ({
      value: i + 1,
      label: `${i + 1}`,
    }));

  return (
    <Dialog open={open} onClose={() => setOpen(false)}>
      <DialogTitle>Ny Afstemning</DialogTitle>
      <DialogContent>
        <Stack spacing={2}>
          {!['vote/policy', 'vote/change'].includes(query?.mimeId ?? '') &&
            optionsCount > 2 && (
              <>
                <Typography>Stemmeinterval</Typography>
                <Slider
                  value={voteCount}
                  onChange={handleChange}
                  valueLabelDisplay="off"
                  min={1}
                  marks={getMarks(optionsCount)}
                  max={optionsCount - 1}
                />
              </>
            )}
          <FormControlLabel
            control={
              <Switch
                checked={hidden}
                onChange={() => setHidden(!hidden)}
                color="primary"
              />
            }
            label="Skjul resultatet"
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button
          endIcon={<PlayArrow />}
          variant="contained"
          color="primary"
          disabled={loading}
          onClick={handleAddPoll}
        >
          Start
        </Button>
      </DialogActions>
    </Dialog>
  );
}

export default PollDialog;