import {
  CardActions,
  Popover,
  Paper,
  Slider,
  Card,
  CardHeader,
} from '@mui/material';
import { Visibility, VisibilityOff, ZoomIn } from '@mui/icons-material';
import { Node, useSession } from 'hooks';
import { AutoButton, MimeIcon } from 'comps';
import { useState } from 'react';

const marks = [
  {
    value: 50,
    label: '50%',
  },
  {
    value: 100,
    label: '100%',
  },
  {
    value: 150,
    label: '150%',
  },
  {
    value: 200,
    label: '200%',
  },
  {
    value: 250,
    label: '250%',
  },
];

const ScreenToolCard = ({ node }: { node: Node }) => {
  const [anchorEl, setAnchorEl] = useState<Element | null>(null);
  const open = Boolean(anchorEl);
  const [_, setSession] = useSession();
  const context = node.useContext();
  const set = context.useSet();

  const handleFocus = (id?: string | null) => () => {
    if (id === undefined) return;
    set('active', id);
  };

  return (
    <>
      <Popover
        key="zoom-slider"
        open={open}
        anchorEl={anchorEl}
        anchorOrigin={{
          vertical: 'bottom',
          horizontal: 'center',
        }}
        transformOrigin={{
          vertical: 'top',
          horizontal: 'center',
        }}
        onClose={() => setAnchorEl(null)}
      >
        <Paper>
          <Slider
            sx={{ width: 200, margin: 3 }}
            min={50}
            max={250}
            defaultValue={200}
            step={50}
            marks={marks}
            onChange={(_, newValue) =>
              setSession({ screen: { size: `${newValue}%` } })
            }
          />
        </Paper>
      </Popover>
      <Card elevation={0}>
        <CardHeader avatar={<MimeIcon mimeId="app/screen" />} title="Skærm" />
        <CardActions>
          <AutoButton
            key="focus"
            text="Vis"
            icon={<Visibility />}
            onClick={handleFocus(node.id)}
          />
          <AutoButton
            key="hide"
            text="Skjul"
            icon={<VisibilityOff />}
            onClick={handleFocus(null)}
          />
          <AutoButton
            key="zoom"
            text="Zoom"
            icon={<ZoomIn />}
            onClick={(e) => setAnchorEl(e.currentTarget)}
          />
        </CardActions>
      </Card>
    </>
  );
};

export default ScreenToolCard;
