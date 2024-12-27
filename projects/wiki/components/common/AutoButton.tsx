import React, { MouseEventHandler } from 'react';
import { Button, useMediaQuery, IconButton, Tooltip } from '@mui/material';

const AutoButton = ({
  text,
  icon,
  onClick,
}: {
  text: string;
  icon: JSX.Element;
  onClick: MouseEventHandler<HTMLButtonElement>;
}) => {
  const largeScreen = useMediaQuery('(min-width:1200px)');

  return largeScreen ? (
    <Button
      color="secondary"
      variant="outlined"
      endIcon={icon}
      onClick={onClick}
    >
      {text}
    </Button>
  ) : (
    <IconButton aria-label={text} color="secondary" onClick={onClick} size="large">
      <Tooltip title={text}>{icon}</Tooltip>
    </IconButton>
  );
};

export default AutoButton;
