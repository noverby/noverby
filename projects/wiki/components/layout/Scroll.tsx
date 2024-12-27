import React from 'react';
import { Box, useMediaQuery } from '@mui/material';

const Scroll = ({ children }: { children: JSX.Element }) => {
  const largeScreen = useMediaQuery('(min-width:1200px)');
  return (
    <Box
      id="scroll"
      sx={{
        // Disable scroll (Firefox)
        scrollbarWidth: 'none',
        // Disable scroll (Webkit)
        '::-webkit-scrollbar': {
          display: 'none',
        },
        overflowY: 'auto',
        WebkitOverflowScrolling: 'touch',
        height: '100%',
        position: 'fixed',
        width: largeScreen ? 'calc(100vw - 416px)' : '100%',
        left: largeScreen ? '472px' : '0px',
        pr: largeScreen ? 8 : 0,
      }}
    >
      <>
        {children}
        <Box sx={{ p: 4 }} />
      </>
    </Box>
  );
};

export default Scroll;
