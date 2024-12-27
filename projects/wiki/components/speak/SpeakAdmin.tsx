import React, { useState } from 'react';
import { Button, ButtonGroup, TextField } from '@mui/material';
import { AdminCard } from 'comps';
import { Clear, LockOpen, Lock, PlayArrow, Stop } from '@mui/icons-material';
import { Node } from 'hooks';

const SpeakAdmin = ({ node, time }: { node: Node; time: number }) => {
  const get = node.useSubsGet();
  const speakerlist = get('speakerlist');
  const update = node.useUpdate({ refetch: false });
  const children = node.useChildren();
  const [timeBox, setTimeBox] = useState(120);
  const id = speakerlist?.id;

  const handleRemoveSpeaks = () => {
    children.delete({
      _and: [
        { mimeId: { _eq: 'speak/speak' } },
        {
          parentId: { _eq: id },
        },
      ],
    });
  };

  const handleLockSpeak = (mutable: boolean) => {
    update({ id, set: { mutable } });
  };

  const handleTimerSet = (time: number) => {
    const updatedAt = new Date();
    update({ id, set: { data: { time, updatedAt } } });
  };

  const owner = speakerlist?.isContextOwner;
  const mutable = speakerlist?.mutable;

  return (
    (owner && (
      <AdminCard title="Administrer Talerlisten">
        <ButtonGroup variant="contained" sx={{ m: 2, boxShadow: 0 }}>
          <Button
            color="secondary"
            size="large"
            endIcon={mutable ? <Lock /> : <LockOpen />}
            onClick={() => handleLockSpeak(!mutable)}
          >
            {mutable ? 'Luk' : 'Åben'}
          </Button>

          <Button
            color="secondary"
            size="large"
            endIcon={<Clear />}
            onClick={handleRemoveSpeaks}
          >
            Ryd
          </Button>
          <Button
            color="secondary"
            size="large"
            endIcon={time == 0 ? <PlayArrow /> : <Stop />}
            onClick={() => handleTimerSet(time > 0 ? 0 : timeBox)}
          >
            {time == 0 ? 'Start' : 'Stop'}
          </Button>
        </ButtonGroup>

        <TextField
          id="filled-number"
          label="Taletid"
          type="number"
          color="secondary"
          value={timeBox}
          sx={{
            bgcolor: 'secondary.main',
            borderColor: 'white',
            m: 2,
          }}
          InputLabelProps={{
            shrink: true,
            sx: { color: '#fff' },
          }}
          InputProps={{
            sx: { color: '#fff' },
          }}
          onChange={(e) => setTimeBox(parseInt(e.target.value))}
          variant="filled"
        />
      </AdminCard>
    )) ||
    null
  );
};

export default SpeakAdmin;
