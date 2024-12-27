import React from 'react';
import { AppBar, Toolbar, useMediaQuery, Box, Container } from '@mui/material';
import { HideOnScroll, SearchField } from 'comps';
import { drawerWidth } from 'core/constants';

const BottomBar = ({
  openDrawer,
  setOpenDrawer,
}: {
  openDrawer: boolean;
  setOpenDrawer: (val: boolean) => void;
}) => {
  const largeScreen = useMediaQuery('(min-width:1200px)');

  return (
    <>
      <HideOnScroll>
        <AppBar
          elevation={0}
          sx={
            largeScreen
              ? {
                  width: `calc(100% - ${drawerWidth + 72}px)`,
                  top: 0,
                  background: 'transparent',
                  boxShadow: 'none',
                }
              : {
                  top: 'auto',
                  bottom: 0,
                  background: 'transparent',
                  boxShadow: 'none',
                }
          }
        >
          <Toolbar disableGutters>
            <Box
              sx={{
                position: 'absolute',
                width: largeScreen ? 'calc(100vw - 400px)' : '100%',
                pr: largeScreen ? 10 : 0,
              }}
            >
              <Container
                sx={{
                  pl: 1,
                  pr: 1,
                }}
                disableGutters
              >
                <SearchField
                  openDrawer={openDrawer}
                  setOpenDrawer={setOpenDrawer}
                />
              </Container>
            </Box>
            <Box sx={{ flexGrow: 1 }} />
          </Toolbar>
        </AppBar>
      </HideOnScroll>
      <Box sx={{ p: 4 }} />
    </>
  );
};

export default BottomBar;
