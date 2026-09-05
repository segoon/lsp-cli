# Daemon writer completion events must not use reader admission

The daemon reader admission protocol waits for coordinator acknowledgement before
the producer reads again. Reusing that blocking delivery method for writer
completion events makes the writer stop draining its output queue until each
completion is dispatched. Writer progress events need nonblocking publication;
their acknowledgement sender must also have no live receiver, or unused
acknowledgements accumulate without bound.
