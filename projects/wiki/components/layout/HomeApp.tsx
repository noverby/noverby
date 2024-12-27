import React, { Suspense } from 'react';
import {
  Typography,
  CardContent,
  Grid,
  Avatar,
  Card,
  useMediaQuery,
  Button,
  Stack,
  Box,
} from '@mui/material';
import { AddContentFab, HeaderCard, HomeList, InvitesUserList } from 'comps';
import { useNode } from 'hooks';
import { Hail, HowToReg, Login } from '@mui/icons-material';
import { useAuthenticationStatus, useUserDisplayName } from '@nhost/nextjs';
import { useRouter } from 'next/navigation';

const AddContentFabSuspense = () => {
  const node = useNode();
  return <AddContentFab node={node} />;
};

const HomeApp = () => {
  const router = useRouter();
  const { isAuthenticated } = useAuthenticationStatus();
  const displayName = useUserDisplayName();
  const largeScreen = useMediaQuery('(min-width:1200px)');

  return (
    <Grid direction={largeScreen ? 'row-reverse' : 'row'} container spacing={1}>
      {isAuthenticated && (
        <Grid item xs={12} lg={4}>
          <InvitesUserList />
          <Suspense fallback={null}>
            <AddContentFabSuspense />
          </Suspense>
        </Grid>
      )}
      <Grid item xs={12} lg>
        {isAuthenticated && !largeScreen ? (
          <Card>
            <HomeList />
          </Card>
        ) : (
          <HeaderCard
            title="Velkommen til RadikalWiki"
            avatar={
              <Avatar
                sx={{
                  bgcolor: 'secondary.main',
                }}
              >
                <Hail />
              </Avatar>
            }
          >
            <CardContent>
              {(!isAuthenticated && (
                <>
                  <Typography>Log ind eller registrer dig.</Typography>
                  <Typography>
                    Husk at bruge den email, som du registrerede dig med hos RU.
                  </Typography>
                  <Stack direction="row">
                    <Button
                      startIcon={<Login />}
                      sx={{ mt: 1 }}
                      variant="outlined"
                      onClick={() => router.push('/user/login')}
                    >
                      Log ind
                    </Button>
                    <Box sx={{ p: 1 }} />
                    <Button
                      startIcon={<HowToReg />}
                      sx={{ mt: 1 }}
                      variant="outlined"
                      onClick={() => router.push('/user/register')}
                    >
                      Registrer
                    </Button>
                  </Stack>
                </>
              )) || (
                <>
                  <Typography sx={{ mb: 1 }}>Hej {displayName}!</Typography>
                  <Typography sx={{ mb: 1 }}>
                    Accepter venligst dine invitationer til grupper og
                    begivenheder.
                  </Typography>
                  <Typography>
                    Hvis der ikke forekommer nogen invitationer, så har du højst
                    sandsynligt brugt en anden email, end den som er registreret
                    ved Radikal Ungdom.
                  </Typography>
                </>
              )}
            </CardContent>
          </HeaderCard>
        )}
      </Grid>
    </Grid>
  );
};

export default HomeApp;
